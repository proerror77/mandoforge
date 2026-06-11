use crate::components::{KeyMetrics, Panel, Rows};
use crate::state::ConsoleData;
use crate::{board_column, label_or, short_id};
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct BoardProps {
    pub(crate) data: ConsoleData,
}

#[component]
pub(crate) fn BoardView(props: &BoardProps) -> Html {
    let items = &props.data.task_board.data.items;
    html! {
        <div class="page-stack">
            <div class="kanban">
                { for ["ready", "running", "review", "blocked", "backlog", "done"].iter().map(|column| {
                    let filtered = items.iter().filter(|item| board_column(&item.status) == *column).collect::<Vec<_>>();
                    html! {
                        <section class="board-column">
                            <header>
                                <strong>{ column.to_ascii_uppercase() }</strong>
                                <span>{ filtered.len() }</span>
                            </header>
                            { for filtered.into_iter().map(|item| html! {
                                <article class="board-card" key={item.id.clone()}>
                                    <strong>{ label_or(&item.title, item.work_item.as_ref().map(|w| w.title.as_str()).unwrap_or("Untitled work")) }</strong>
                                    <span>{ format!("{} / {}", label_or(&item.priority, "normal"), short_id(&item.id)) }</span>
                                </article>
                            }) }
                        </section>
                    }
                }) }
            </div>
            <div class="page-grid">
                <Panel title="Work items">
                    <Rows empty="No work items." rows={props.data.work_items.data.iter().take(8).map(|item| {
                        (item.status.clone(), label_or(&item.title, "work item").to_string(), item.priority.clone())
                    }).collect::<Vec<_>>()} />
                </Panel>
                <Panel title="Handoffs and reviews">
                    <KeyMetrics values={vec![
                        ("Manager plans".to_string(), props.data.manager_plans.data.len().to_string()),
                        ("Handoffs".to_string(), props.data.agent_handoffs.data.len().to_string()),
                        ("Assignments".to_string(), props.data.agent_handoff_assignments.data.len().to_string()),
                    ]} />
                </Panel>
            </div>
        </div>
    }
}
