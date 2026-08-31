use crate::api::OntologyReviewGraph;
use js_sys::{Array, Object, Reflect};
use serde::Serialize;
use serde_json::json;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "MandoForgeOntologyGraph"], js_name = mount)]
    fn js_mount_ontology_graph(
        element: &web_sys::Element,
        graph_data: JsValue,
        callbacks: &Object,
    ) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "MandoForgeOntologyGraph"], js_name = update)]
    fn js_update_ontology_graph(instance: &JsValue, graph_data: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "MandoForgeOntologyGraph"], js_name = fit)]
    fn js_fit_ontology_graph(instance: &JsValue);

    #[wasm_bindgen(js_namespace = ["window", "MandoForgeOntologyGraph"], js_name = resetLayout)]
    fn js_reset_ontology_graph(instance: &JsValue, graph_data: JsValue);

    #[wasm_bindgen(js_namespace = ["window", "MandoForgeOntologyGraph"], js_name = selectNodes)]
    fn js_select_ontology_nodes(instance: &JsValue, selected_node_id: &str, selected_ids: JsValue);

    #[wasm_bindgen(js_namespace = ["window", "MandoForgeOntologyGraph"], js_name = destroy)]
    fn js_destroy_ontology_graph(instance: &JsValue);
}

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct OntologyGraphIslandProps {
    pub(crate) graph: OntologyReviewGraph,
    pub(crate) selected_node_id: String,
    pub(crate) selected_node_ids: Vec<String>,
    pub(crate) on_select: Callback<String>,
    pub(crate) on_selection_change: Callback<Vec<String>>,
    pub(crate) fit_label: String,
    pub(crate) reset_label: String,
}

#[component]
pub(crate) fn OntologyGraphIsland(props: &OntologyGraphIslandProps) -> Html {
    let container_ref = use_node_ref();
    let graph_instance = use_mut_ref(|| None::<JsValue>);
    let select_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);
    let selection_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);

    {
        let graph_instance = graph_instance.clone();
        use_effect_with((), move |_| {
            move || {
                if let Some(instance) = graph_instance.borrow_mut().take() {
                    js_destroy_ontology_graph(&instance);
                }
            }
        });
    }

    {
        let container_ref = container_ref.clone();
        let graph_instance = graph_instance.clone();
        let select_closure = select_closure.clone();
        let selection_closure = selection_closure.clone();
        let on_select = props.on_select.clone();
        let on_selection_change = props.on_selection_change.clone();
        let graph = props.graph.clone();
        let selected_node_id = props.selected_node_id.clone();
        let selected_node_ids =
            normalized_selected_ids(&selected_node_id, &props.selected_node_ids);

        use_effect_with(graph.clone(), move |graph| {
            if let Some(element) = container_ref.cast::<web_sys::Element>() {
                let payload = ontology_graph_payload(graph, &selected_node_id, &selected_node_ids);
                let current_instance = graph_instance.borrow().clone();
                if let Some(instance) = current_instance.as_ref() {
                    let updated = js_update_ontology_graph(instance, payload);
                    *graph_instance.borrow_mut() = Some(updated);
                } else {
                    let callbacks = Object::new();

                    let on_select_closure = Closure::<dyn Fn(JsValue)>::wrap(Box::new({
                        let on_select = on_select.clone();
                        move |id: JsValue| {
                            if let Some(id) = id.as_string() {
                                on_select.emit(id);
                            }
                        }
                    }));
                    let _ = Reflect::set(
                        &callbacks,
                        &JsValue::from_str("onSelect"),
                        on_select_closure.as_ref().unchecked_ref(),
                    );
                    *select_closure.borrow_mut() = Some(on_select_closure);

                    let on_selection_closure = Closure::<dyn Fn(JsValue)>::wrap(Box::new({
                        let on_selection_change = on_selection_change.clone();
                        move |ids: JsValue| {
                            let selected = Array::from(&ids)
                                .iter()
                                .filter_map(|value| value.as_string())
                                .collect::<Vec<_>>();
                            on_selection_change.emit(selected);
                        }
                    }));
                    let _ = Reflect::set(
                        &callbacks,
                        &JsValue::from_str("onSelectionChange"),
                        on_selection_closure.as_ref().unchecked_ref(),
                    );
                    *selection_closure.borrow_mut() = Some(on_selection_closure);

                    let instance = js_mount_ontology_graph(&element, payload, &callbacks);
                    if !instance.is_null() && !instance.is_undefined() {
                        *graph_instance.borrow_mut() = Some(instance);
                    }
                }
            }
            || ()
        });
    }

    {
        let graph_instance = graph_instance.clone();
        let selected_node_id = props.selected_node_id.clone();
        let selected_node_ids =
            normalized_selected_ids(&selected_node_id, &props.selected_node_ids);
        use_effect_with(
            (selected_node_id, selected_node_ids),
            move |(selected_node_id, selected_node_ids)| {
                if let Some(instance) = graph_instance.borrow().as_ref() {
                    if let Ok(ids) = selected_node_ids
                        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
                    {
                        js_select_ontology_nodes(instance, selected_node_id, ids);
                    }
                }
                || ()
            },
        );
    }

    let fit_graph = {
        let graph_instance = graph_instance.clone();
        Callback::from(move |_| {
            if let Some(instance) = graph_instance.borrow().as_ref() {
                js_fit_ontology_graph(instance);
            }
        })
    };
    let reset_graph = {
        let graph_instance = graph_instance.clone();
        let graph = props.graph.clone();
        let selected_node_id = props.selected_node_id.clone();
        let selected_node_ids =
            normalized_selected_ids(&selected_node_id, &props.selected_node_ids);
        Callback::from(move |_| {
            if let Some(instance) = graph_instance.borrow().as_ref() {
                let payload = ontology_graph_payload(&graph, &selected_node_id, &selected_node_ids);
                js_reset_ontology_graph(instance, payload);
            }
        })
    };

    html! {
        <section class="ontology-network-canvas" aria-label="Ontology knowledge graph">
            <div class="ontology-graph-toolbar" aria-label="Graph controls">
                <button class="secondary" type="button" onclick={fit_graph}>{ props.fit_label.clone() }</button>
                <button class="secondary" type="button" onclick={reset_graph}>{ props.reset_label.clone() }</button>
            </div>
            <div class="ontology-cytoscape-host" ref={container_ref}></div>
        </section>
    }
}

fn normalized_selected_ids(selected_node_id: &str, selected_node_ids: &[String]) -> Vec<String> {
    if selected_node_ids.is_empty() && !selected_node_id.is_empty() {
        return vec![selected_node_id.to_string()];
    }
    selected_node_ids.to_vec()
}

fn ontology_graph_payload(
    graph: &OntologyReviewGraph,
    selected_node_id: &str,
    selected_node_ids: &[String],
) -> JsValue {
    json!({
        "run_id": &graph.run_id,
        "nodes": &graph.nodes,
        "edges": &graph.edges,
        "selected_node_id": selected_node_id,
        "selected_ids": selected_node_ids,
        "truncated": graph.truncated,
        "omitted_node_count": graph.omitted_node_count,
        "omitted_edge_count": graph.omitted_edge_count,
    })
    .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
    .unwrap_or(JsValue::NULL)
}
