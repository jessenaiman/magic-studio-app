use super::*;

fn build_steps(recipe: &Value, all: &[Line], duration: f64) -> (Vec<Value>, Vec<Value>) {
    build_steps_with_designs(recipe, &json!({}), all, duration)
}

fn aligned() -> Value {
    let w = |line: &str, text: &str, t: Option<(f64, f64)>| {
        let (s, e) = t.map_or((Value::Null, Value::Null), |(s, e)| (json!(s), json!(e)));
        json!({"sung_id": line, "text": text, "start": s, "end": e})
    };
    json!({"words": [
        w("l1", "X", None), w("l1", "marks", Some((0.0, 0.8))), w("l1", "the", Some((0.8, 1.2))), w("l1", "spot,", Some((1.26, 1.82))),
        w("l2", "And", Some((3.5, 3.9))), w("l2", "Dash", Some((4.1, 4.5))),
    ]})
}

fn recipe() -> Value {
    json!({"band": "top", "max_heroes": 1, "palette": {"ink": "#173F49", "rust": "#B8442A", "cream": "#FFF8E9"},
        "base": {"lettering": "sans-bold", "size": 58, "color": "ink", "outline": "cream"},
        "heroes": {"spot": {"lettering": "serif-bold", "size": 130, "color": "rust", "motion": "pop"}}})
}

#[test]
fn a_hero_is_styled_and_animated_on_its_own_characters_at_its_sung_time() {
    let all = lines(&aligned());
    assert_eq!(all[0].text(), "X marks the spot,");
    assert_eq!(all[0].range(3), (12, 17));
    let (steps, summary) = build_steps(&recipe(), &all, 33.0);
    let styled = steps
        .iter()
        .find(|s| s["command"] == "layer.setText")
        .unwrap();
    assert_eq!(styled["params"]["range"], json!([12, 17]));
    assert_eq!(styled["params"]["font"], "C059");
    let pop: Vec<f64> = steps
        .iter()
        .filter(|s| s["params"]["path"] == "text/animators/animator/properties/scale")
        .filter_map(|s| s["params"]["time"].as_f64())
        .collect();
    assert_eq!(pop.len(), 3);
    assert!((pop[1] - 1.32).abs() < 1e-9);
    assert_eq!(summary[0]["hero"]["word"], "spot,");
}

#[test]
fn lines_show_before_they_are_sung_and_never_overlap() {
    let (_, summary) = build_steps(&recipe(), &lines(&aligned()), 33.0);
    let (out0, in1) = (
        summary[0]["out"].as_f64().unwrap(),
        summary[1]["in"].as_f64().unwrap(),
    );
    assert!(summary[0]["in"].as_f64().unwrap() == 0.0);
    assert!(in1 >= out0, "{out0} {in1}");
    assert!((in1 - 3.25).abs() < 1e-9);
}

#[test]
fn spatial_line_design_places_each_item_and_rotates_and_animates_trim_strokes() {
    let designs = json!({
        "l1": {
            "items": [
                {
                    "text": "X marks the",
                    "font": "sans",
                    "weight": 700,
                    "italic": false,
                    "size": 58.0,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "x": 344.0,
                    "y": 97.0,
                    "rotate": 0.0,
                    "motion": "quiet",
                    "word_ids": ["l1/w0", "l1/w1", "l1/w2"]
                },
                {
                    "text": "spot,",
                    "font": "serif",
                    "weight": 700,
                    "italic": false,
                    "size": 130.0,
                    "color": "#B8442A",
                    "outline": "#FFF8E9",
                    "x": 708.0,
                    "y": 30.0,
                    "rotate": -7.0,
                    "motion": "pop",
                    "accent": "l1/w3",
                    "word_ids": ["l1/w3"]
                }
            ],
            "draws": [
                {
                    "name": "Underline",
                    "view_box": "0 0 280 30",
                    "x": 754.0,
                    "y": 168.0,
                    "w": 253.0,
                    "h": 27.0,
                    "paths": [
                        {
                            "d": "M 5 14 Q 61 5 125 13 T 269 10",
                            "stroke": "#B8442A",
                            "width": 7.0,
                            "from": 0.59,
                            "to": 1.14
                        }
                    ]
                }
            ]
        }
    });
    let (steps, _) = build_steps_with_designs(&recipe(), &designs, &lines(&aligned()), 33.0);
    let texts: Vec<&Value> = steps
        .iter()
        .filter(|s| s["command"] == "layer.newText")
        .collect();
    assert_eq!(texts.len(), 3);
    assert_eq!(texts[0]["params"]["name"], "l1_item0");
    assert_eq!(texts[0]["params"]["box"][0], 344.0);
    assert_eq!(texts[0]["params"]["box"][1], 97.0);
    assert_eq!(texts[1]["params"]["name"], "l1_item1");
    assert_eq!(texts[1]["params"]["box"][0], 708.0);
    assert_eq!(texts[1]["params"]["box"][1], 30.0);
    let rot = steps
        .iter()
        .find(|s| s["params"]["layer"] == "l1_item1" && s["params"]["path"] == "transform/rotation")
        .unwrap();
    assert_eq!(rot["params"]["value"], -7.0);
    let svg = steps
        .iter()
        .find(|s| s["command"] == "edit.pasteSvg")
        .unwrap();
    assert_eq!(svg["params"]["name"], "l1_draw0_0");
    let trim_keys: Vec<(f64, f64)> = steps
        .iter()
        .filter(|s| {
            s["params"]["layer"] == "l1_draw0_0" && s["params"]["path"] == "contents/trim/end"
        })
        .map(|s| {
            (
                s["params"]["time"].as_f64().unwrap(),
                s["params"]["value"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(trim_keys, vec![(0.59, 0.0), (1.14, 100.0)]);
}

#[test]
fn recipe_hero_stroke_emits_svg_trim_shape_layer() {
    let mut r = recipe();
    r["heroes"]["spot"]["stroke"] = json!("underline");
    let (steps, _) = build_steps(&r, &lines(&aligned()), 33.0);
    let svg = steps
        .iter()
        .find(|s| s["command"] == "edit.pasteSvg")
        .unwrap();
    assert_eq!(svg["params"]["name"], "l1_stroke");
    let trim_item = steps
        .iter()
        .find(|s| s["command"] == "layer.addShapeItem")
        .unwrap();
    assert_eq!(trim_item["params"]["layer"], "l1_stroke");
    assert_eq!(trim_item["params"]["kind"], "trim");
}

#[test]
fn runtime_words_tools_queries_effectcraft_and_filmcraft_catalogs() {
    let lib: Value = serde_json::from_str(crate::cmd::words::KINETIC_LIBRARY).unwrap();
    let tools = crate::cmd::words::stage::runtime_words_tools(&lib);
    assert!(tools["moves"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(tools["strokes"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(
        tools["effectcraft"]["text_presets"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
    assert!(
        tools["effectcraft"]["paint_presets"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
    assert!(
        tools["effectcraft"]["effects"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
    assert!(
        tools["filmcraft"]["effects"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
}
