use anyhow::{Context, Result, ensure};
use std::collections::HashSet;

use crate::vm::schema::{NetworkAdapter, NetworkAdapters, NetworkMode};

use super::{Action, NetworkAdapterLabel, NetworkAttachment, Plan, Snapshot, external_id};

struct PlanningState {
    unclaimed_attachments: Vec<NetworkAttachment>,
    occupied_labels: HashSet<NetworkAdapterLabel>,
    pending_adapters: Vec<(String, NetworkAdapter)>,
    configure_actions: Vec<Action>,
}

pub(crate) fn plan(configured: &NetworkAdapters, snapshot: Snapshot) -> Result<Plan> {
    let state = configured
        .iter()
        .try_fold(PlanningState::new(snapshot), |state, (name, adapter)| {
            state.match_adapter(name, adapter)
        })?;

    Ok(state.assign_pending_adapters()?.into_plan())
}

impl PlanningState {
    fn new(snapshot: Snapshot) -> Self {
        let unclaimed_attachments = snapshot.network_attachments;
        let occupied_labels = unclaimed_attachments
            .iter()
            .map(|adapter| adapter.label.clone())
            .collect();

        Self {
            unclaimed_attachments,
            occupied_labels,
            pending_adapters: Vec::new(),
            configure_actions: Vec::new(),
        }
    }

    fn match_adapter(mut self, name: &str, adapter: &NetworkAdapter) -> Result<Self> {
        validate_adapter(name, adapter)?;

        let expected_id = external_id(name);
        let attached = self
            .unclaimed_attachments
            .iter()
            .position(|attached| attached.external_id == expected_id)
            .map(|index| self.unclaimed_attachments.remove(index));

        match attached {
            Some(attached) => {
                if !matches_configuration(&attached, adapter) {
                    self.configure_actions.push(Action::Configure {
                        label: attached.label,
                        name: name.to_owned(),
                        adapter: adapter.clone(),
                    });
                }
            }
            None => self
                .pending_adapters
                .push((name.to_owned(), adapter.clone())),
        }

        Ok(self)
    }

    fn assign_pending_adapters(mut self) -> Result<Self> {
        let pending_adapters = std::mem::take(&mut self.pending_adapters);

        pending_adapters
            .into_iter()
            .try_fold(self, |mut state, (name, adapter)| {
                // Reuse an unclaimed adapter before allocating a new VMware label.
                let label = if state.unclaimed_attachments.is_empty() {
                    let label = first_free_label(&state.occupied_labels)?;
                    state.occupied_labels.insert(label.clone());
                    label
                } else {
                    state.unclaimed_attachments.remove(0).label
                };

                state.configure_actions.push(Action::Configure {
                    label,
                    name,
                    adapter,
                });

                Ok(state)
            })
    }

    fn into_plan(self) -> Plan {
        let remove_actions = self
            .unclaimed_attachments
            .into_iter()
            .map(|adapter| Action::Remove {
                label: adapter.label,
            });

        Plan {
            actions: remove_actions.chain(self.configure_actions).collect(),
        }
    }
}

fn validate_adapter(name: &str, adapter: &NetworkAdapter) -> Result<()> {
    let valid_vmnet = match adapter.mode {
        NetworkMode::Custom => adapter
            .vmnet
            .as_deref()
            .is_some_and(|vmnet| !vmnet.is_empty()),
        _ => adapter.vmnet.is_none(),
    };

    ensure!(
        valid_vmnet,
        "network adapter {name} must specify a non-empty vmnet when its mode is custom and omit it otherwise"
    );

    Ok(())
}

fn first_free_label(occupied_labels: &HashSet<NetworkAdapterLabel>) -> Result<NetworkAdapterLabel> {
    // VMware Fusion supports up to ten adapters, labeled ethernet0 through ethernet9.
    (0..10)
        .map(|index| format!("ethernet{index}"))
        .find(|label| !occupied_labels.contains(label))
        .context("virtual machine already has the maximum of ten network adapters")
}

fn matches_configuration(attached: &NetworkAttachment, configured: &NetworkAdapter) -> bool {
    let vmnet_matches = configured.mode != NetworkMode::Custom
        || configured.vmnet.as_deref() == Some(attached.vmnet.as_str());

    attached.mode == configured.mode.vmcli_value()
        && vmnet_matches
        && attached.model == configured.model.vmcli_value()
        && attached.start_connected == configured.start_connected
}

#[cfg(test)]
mod tests {
    use crate::vm::schema::NetworkAdapterModel;

    use super::*;

    fn configured_adapters(name: &str, mode: NetworkMode) -> NetworkAdapters {
        NetworkAdapters::from([(name.to_owned(), configured_adapter(mode))])
    }

    fn configured_adapter(mode: NetworkMode) -> NetworkAdapter {
        NetworkAdapter {
            mode,
            vmnet: None,
            model: NetworkAdapterModel::Vmxnet3,
            start_connected: true,
        }
    }

    fn network_attachment(label: &str, name: &str, mode: &str) -> NetworkAttachment {
        NetworkAttachment {
            label: label.to_owned(),
            external_id: external_id(name),
            mode: mode.to_owned(),
            vmnet: String::new(),
            model: "vmxnet3".to_owned(),
            start_connected: true,
        }
    }

    #[test]
    fn matching_custom_network_adapter_is_unchanged() -> Result<()> {
        let mut adapter = configured_adapter(NetworkMode::Custom);
        adapter.vmnet = Some("vmnet2".to_owned());

        let configured = NetworkAdapters::from([("primary".to_owned(), adapter)]);

        let mut attachment = network_attachment("ethernet0", "primary", "custom");
        attachment.vmnet = "vmnet2".to_owned();

        let snapshot = Snapshot {
            network_attachments: vec![attachment],
        };

        assert!(plan(&configured, snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn changed_network_adapter_mode_is_configured() -> Result<()> {
        let configured = configured_adapters("primary", NetworkMode::HostOnly);
        let snapshot = Snapshot {
            network_attachments: vec![network_attachment("ethernet0", "primary", "nat")],
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure {
                label,
                name,
                adapter,
            }] if label == "ethernet0"
                && name == "primary"
                && adapter.mode == NetworkMode::HostOnly
        ));

        Ok(())
    }

    #[test]
    fn changed_custom_vmnet_is_configured() -> Result<()> {
        let mut adapter = configured_adapter(NetworkMode::Custom);
        adapter.vmnet = Some("vmnet3".to_owned());

        let configured = NetworkAdapters::from([("primary".to_owned(), adapter)]);

        let mut attachment = network_attachment("ethernet0", "primary", "custom");
        attachment.vmnet = "vmnet2".to_owned();

        let snapshot = Snapshot {
            network_attachments: vec![attachment],
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure { label, name, .. }]
                if label == "ethernet0" && name == "primary"
        ));

        Ok(())
    }

    #[test]
    fn missing_network_adapter_is_configured() -> Result<()> {
        let configured = configured_adapters("primary", NetworkMode::Nat);
        let snapshot = Snapshot {
            network_attachments: Vec::new(),
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure { label, name, .. }]
                if label == "ethernet0" && name == "primary"
        ));

        Ok(())
    }

    #[test]
    fn existing_network_adapter_is_adopted() -> Result<()> {
        let configured = configured_adapters("primary", NetworkMode::Nat);
        let mut unmanaged_attachment = network_attachment("ethernet0", "", "bridged");
        unmanaged_attachment.external_id.clear();

        let snapshot = Snapshot {
            network_attachments: vec![unmanaged_attachment],
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure { label, name, .. }]
                if label == "ethernet0" && name == "primary"
        ));

        Ok(())
    }

    #[test]
    fn undeclared_network_adapter_is_removed() -> Result<()> {
        let configured = NetworkAdapters::new();
        let snapshot = Snapshot {
            network_attachments: vec![network_attachment("ethernet0", "primary", "nat")],
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Remove { label }] if label == "ethernet0"
        ));

        Ok(())
    }

    #[test]
    fn adapter_names_preserve_identity_across_device_order() -> Result<()> {
        let configured = NetworkAdapters::from([
            ("primary".to_owned(), configured_adapter(NetworkMode::Nat)),
            (
                "private".to_owned(),
                configured_adapter(NetworkMode::HostOnly),
            ),
        ]);
        let snapshot = Snapshot {
            network_attachments: vec![
                network_attachment("ethernet0", "private", "hostonly"),
                network_attachment("ethernet1", "primary", "nat"),
            ],
        };

        assert!(plan(&configured, snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn invalid_vmnet_configuration_is_rejected() {
        let mut non_custom = configured_adapter(NetworkMode::Nat);
        non_custom.vmnet = Some(String::new());

        let cases = [
            (
                "custom adapter without a vmnet",
                configured_adapter(NetworkMode::Custom),
            ),
            ("non-custom adapter with an empty vmnet", non_custom),
        ];

        cases.into_iter().for_each(|(case, adapter)| {
            let error = validate_adapter("primary", &adapter).expect_err(case);

            assert!(error.to_string().contains("must specify a non-empty vmnet"));
        });
    }
}
