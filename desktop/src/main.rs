use slint::{ModelRc, VecModel};
use soundcore_models::catalog::{self, FormFactor, ModelFamily, model as catalog_model};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    ui.set_forms(ModelRc::new(VecModel::from(build_forms())));

    let weak = ui.as_weak();
    ui.on_select(move |code| {
        if let Some(ui) = weak.upgrade() {
            ui.set_detail(detail_for(&code).into());
        }
    });

    ui.run()
}

fn build_forms() -> Vec<CatalogForm> {
    catalog::grouped_families()
        .into_iter()
        .map(|(form_factor, families)| {
            let count: usize = families.iter().map(|family| family.models().count()).sum();
            let families = families.into_iter().map(family_to_ui).collect::<Vec<_>>();
            CatalogForm {
                title: form_factor.title_ru().into(),
                subtitle: format!("{count} моделей · {}", subtitle(form_factor)).into(),
                families: ModelRc::new(VecModel::from(families)),
            }
        })
        .collect()
}

fn family_to_ui(family: &'static ModelFamily) -> CatalogFamily {
    let models = family
        .models()
        .map(|model| CatalogModel {
            code: model.code.into(),
            name: model.name.into(),
            battery: model.battery.title_ru().into(),
            confidence: if model.confidence == catalog::Confidence::Inferred {
                "проверить".into()
            } else {
                "".into()
            },
        })
        .collect::<Vec<_>>();
    CatalogFamily {
        title: family.title.into(),
        tier: family.tier.title_ru().into(),
        models: ModelRc::new(VecModel::from(models)),
    }
}

fn subtitle(form_factor: FormFactor) -> &'static str {
    match form_factor {
        FormFactor::TwsEarbuds => "два наушника и кейс",
        FormFactor::Headphones => "оголовье, одна батарея",
        FormFactor::OpenEar => "клипсы и заушные",
        FormFactor::Speaker => "портативная акустика",
        FormFactor::SleepBuds => "вкладыши для сна",
    }
}

fn detail_for(code: &str) -> String {
    let Some(model) = catalog_model(code) else {
        return "Модель не найдена в каталоге.".to_owned();
    };
    let Some(family) = catalog::family(model.family) else {
        return format!("{}: семейство не найдено.", model.name);
    };
    let capabilities = family
        .shared
        .iter()
        .map(|capability| capability.title_ru())
        .collect::<Vec<_>>()
        .join(", ");
    let parser = if soundcore_models::model_by_code(model.code).is_some() {
        "разбор состояния: реализован"
    } else {
        "разбор состояния: в разработке"
    };
    format!(
        "{}\nКод: {}\nФорм-фактор: {}\nСемейство: {}\nУровень: {}\nБатарея: {}\nУверенность: {}\n{}\n\nОбщие признаки семейства:\n{}\n\n{}",
        model.name,
        model.code,
        family.form_factor.title_ru(),
        family.title,
        family.tier.title_ru(),
        model.battery.title_ru(),
        model.confidence.title_ru(),
        parser,
        if capabilities.is_empty() {
            "—".to_owned()
        } else {
            capabilities
        },
        family.evidence,
    )
}
