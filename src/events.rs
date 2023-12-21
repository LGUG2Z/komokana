use crate::configuration::Strategy;
#[cfg(feature = "virtual_keys")]
use crate::Komokana;
#[cfg(feature = "virtual_keys")]
use crate::Provider;
use crate::{CONFIG, DEFAULT_LAYER, KANATA};
use color_eyre::Result;
use serde_json::json;
use std::io::Write;

#[derive(Debug, Copy, Clone)]
pub enum Event {
    Show,
    FocusChange,
}

pub fn handle_event(event: Event, exe: &str, title: Option<&str>) -> Result<()> {
    let target = calculate_target(
        event,
        exe,
        title,
        if matches!(event, Event::FocusChange) {
            Option::from(DEFAULT_LAYER.get().unwrap().as_ref())
        } else {
            None
        },
    );

    if let Some(target) = target {
        let stream = &mut KANATA.get().unwrap().get_stream();
        let mut stream = stream.lock();
        let request = json!({
            "ChangeLayer": {
                "new": target,
            }
        });

        stream.write_all(request.to_string().as_bytes())?;
        log::debug!("request sent: {request}");
    };

    Ok(())
}

fn calculate_target(
    event: Event,
    exe: &str,
    title: Option<&str>,
    default: Option<&str>,
) -> Option<String> {
    let configuration = CONFIG.get().unwrap();
    let mut new_layer = default;
    for entry in configuration {
        let entry_strategy = entry.strategy.clone().unwrap_or(Strategy::Equals);
        if matches_with_strategy(exe, &entry.exe, &entry_strategy) {
            if matches!(event, Event::FocusChange) {
                new_layer = Option::from(entry.target_layer.as_str());
            }

            if let Some(title) = title {
                if let Some(title_overrides) = &entry.title_overrides {
                    for title_override in title_overrides {
                        if matches_with_strategy( title, &title_override.title, &title_override.strategy) {
                            new_layer = Option::from(title_override.target_layer.as_str());
                        }
                    }

                    // This acts like a default target layer within the application
                    // which defaults back to the entry's main target layer
                    if new_layer.is_none() {
                        new_layer = Option::from(entry.target_layer.as_str());
                    }
                }
            }

            #[cfg(feature = "virtual_keys")]
            if matches!(event, Event::FocusChange) {
                if let Some(virtual_key_overrides) = &entry.virtual_key_overrides {
                    for virtual_key_override in virtual_key_overrides {
                        if Komokana::get_key_state(virtual_key_override.virtual_key_code) < 0 {
                            new_layer = Option::from(virtual_key_override.targer_layer.as_str());
                        }
                    }
                }

                if let Some(virtual_key_ignores) = &entry.virtual_key_ignores {
                    for virtual_key in virtual_key_ignores {
                        if Komokana::get_key_state(*virtual_key) < 0 {
                            new_layer = None;
                        }
                    }
                }
            }
        }
    }

    new_layer.and_then(|new_layer| Option::from(new_layer.to_string()))
}

fn matches_with_strategy(title: &str, config_title: &str, strategy: &Strategy) -> bool {
    match strategy {
        Strategy::StartsWith => title.starts_with(config_title),
        Strategy::EndsWith => title.ends_with(config_title),
        Strategy::Contains => title.contains(config_title),
        Strategy::Equals => title.eq(config_title),
    }
}
