//! UI module: view dispatcher and screen components.

pub mod details;
pub mod forecast;
pub mod hero;
pub mod splash;
pub mod style;

use ratatui::Frame;
use weathertrack_core::app::Model;

/// Render the current page based on the model state.
pub fn view(f: &mut Frame, model: &Model) {
    let total = model.total_pages();
    match model.page {
        0 => {
            if let Some(ref forecast) = model.forecast {
                if let Some(ref current) = forecast.current {
                    hero::draw(f, current, &model.location, model.page, total);
                }
            }
        }
        1 => {
            if let Some(ref forecast) = model.forecast {
                if let Some(ref current) = forecast.current {
                    details::draw(f, current, model.page, total);
                }
            }
        }
        p => {
            if let Some(ref forecast) = model.forecast {
                if let Some(day) = forecast.daily.get(p - 2) {
                    forecast::draw(f, day, model.page, total);
                }
            }
        }
    }
}
