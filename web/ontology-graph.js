(function () {
  function classFor(value) {
    return String(value || "unknown").replace(/[^a-zA-Z0-9_-]/g, "-");
  }

  function confidenceLabel(value) {
    var number = Number(value || 0);
    return Math.round(number * 100) + "%";
  }

  function buildElements(graphData) {
    var nodes = Array.isArray(graphData.nodes) ? graphData.nodes : [];
    var edges = Array.isArray(graphData.edges) ? graphData.edges : [];
    return nodes.map(function (node) {
      return {
        group: "nodes",
        data: {
          id: node.id,
          label: node.label || node.id,
          typeLabel: node.type_label || node.node_type || "node",
          nodeType: node.node_type || "node",
          status: node.status || "pending",
          confidence: confidenceLabel(node.confidence),
          riskLabel: node.risk_label || node.risk || "low",
          sourceProposalId: node.source_proposal_id || ""
        },
        classes: [
          classFor(node.node_type),
          "status-" + classFor(node.status),
          "risk-" + classFor(node.risk)
        ].join(" ")
      };
    }).concat(edges.map(function (edge) {
      return {
        group: "edges",
        data: {
          id: edge.id || edge.from + "--" + edge.to + "--" + edge.edge_type,
          source: edge.from,
          target: edge.to,
          label: edge.type_label || edge.edge_type || "link",
          edgeType: edge.edge_type || "link",
          status: edge.status || "pending",
          confidence: confidenceLabel(edge.confidence),
          riskLabel: edge.risk_label || edge.risk || "low",
          sourceProposalId: edge.source_proposal_id || ""
        },
        classes: [
          classFor(edge.edge_type),
          "status-" + classFor(edge.status),
          "risk-" + classFor(edge.risk)
        ].join(" ")
      };
    }));
  }

  function makeStyles() {
    return [
      {
        selector: "core",
        style: {
          "selection-box-color": "#24302c",
          "selection-box-border-color": "#24302c",
          "selection-box-opacity": 0.1
        }
      },
      {
        selector: "node",
        style: {
          "shape": "round-rectangle",
          "width": 96,
          "height": 38,
          "padding": "12px",
          "background-color": "#ffffff",
          "border-width": 1,
          "border-color": "#d7d5cc",
          "label": "data(label)",
          "font-size": 11,
          "font-weight": 700,
          "color": "#24302c",
          "text-wrap": "wrap",
          "text-max-width": 118,
          "text-valign": "center",
          "text-halign": "center",
          "overlay-opacity": 0,
        }
      },
      {
        selector: "node.dataset",
        style: {
          "background-color": "#eef5ff",
          "border-color": "#2e6fbd"
        }
      },
      {
        selector: "node.object, node.subgraph, node.merge_candidate",
        style: {
          "background-color": "#ffffff",
          "border-width": 2,
          "border-color": "#24302c",
          "font-size": 12
        }
      },
      {
        selector: "node.metric, node.logic",
        style: {
          "background-color": "#fff7e8",
          "border-color": "#b27416"
        }
      },
      {
        selector: "node.action, node.tool",
        style: {
          "background-color": "#edf8f2",
          "border-color": "#11785f"
        }
      },
      {
        selector: "node.status-approved, node.status-materialized",
        style: {
          "border-style": "solid",
          "border-color": "#11785f"
        }
      },
      {
        selector: "node.status-rejected, node.risk-high, node.risk-blocked",
        style: {
          "border-color": "#b23b2e",
          "background-color": "#fff1ef"
        }
      },
      {
        selector: "node:selected",
        style: {
          "border-width": 3,
          "border-color": "#0e1614",
          "z-index": 20
        }
      },
      {
        selector: "node.faded",
        style: {
          "opacity": 0.22
        }
      },
      {
        selector: "edge",
        style: {
          "curve-style": "bezier",
          "width": 1.6,
          "line-color": "#b8b4aa",
          "target-arrow-shape": "triangle",
          "target-arrow-color": "#b8b4aa",
          "arrow-scale": 0.8,
          "label": "data(label)",
          "font-size": 8,
          "color": "#5f665f",
          "text-background-color": "#fffdf8",
          "text-background-opacity": 0.86,
          "text-background-padding": "2px",
          "text-rotation": "autorotate",
          "overlay-opacity": 0
        }
      },
      {
        selector: "edge.maps_to",
        style: {
          "line-color": "#2e6fbd",
          "target-arrow-color": "#2e6fbd"
        }
      },
      {
        selector: "edge.acts_on, edge.compiles_to",
        style: {
          "line-color": "#11785f",
          "target-arrow-color": "#11785f"
        }
      },
      {
        selector: "edge.uses_metric, edge.validates",
        style: {
          "line-color": "#b27416",
          "target-arrow-color": "#b27416"
        }
      },
      {
        selector: "edge:selected, edge.focused",
        style: {
          "width": 3,
          "line-color": "#0e1614",
          "target-arrow-color": "#0e1614",
          "z-index": 20
        }
      },
      {
        selector: "edge.faded",
        style: {
          "opacity": 0.15
        }
      }
    ];
  }

  function makeLayout(graphData) {
    var nodeCount = Array.isArray(graphData.nodes) ? graphData.nodes.length : 0;
    if (nodeCount <= 14) {
      return {
        name: "concentric",
        animate: false,
        fit: true,
        padding: 36,
        minNodeSpacing: 42,
        nodeDimensionsIncludeLabels: true,
        concentric: function (node) {
          var type = node.data("nodeType");
          if (type === "object" || type === "subgraph") return 5;
          if (type === "dataset") return 2;
          if (type === "tool" || type === "action") return 2;
          return 3;
        },
        levelWidth: function () {
          return 1;
        }
      };
    }
    return {
      name: "cose",
      animate: false,
      fit: true,
      padding: 40,
      nodeDimensionsIncludeLabels: true,
      nodeRepulsion: 9000,
      idealEdgeLength: 115,
      edgeElasticity: 90,
      gravity: 0.16,
      numIter: 900
    };
  }

  function applySelection(cy, graphData) {
    var selectedIds = Array.isArray(graphData.selected_ids)
      ? graphData.selected_ids
      : [];
    cy.batch(function () {
      cy.nodes().unselect();
      selectedIds.forEach(function (id) {
        cy.getElementById(id).select();
      });
    });
  }

  function focusNeighborhood(cy, id) {
    cy.elements().removeClass("faded focused");
    if (!id) return;
    var node = cy.getElementById(id);
    if (!node || node.empty()) return;
    var neighborhood = node.closedNeighborhood();
    cy.elements().not(neighborhood).addClass("faded");
    node.connectedEdges().addClass("focused");
  }

  function emitSelection(cy, callbacks) {
    if (!callbacks || typeof callbacks.onSelectionChange !== "function") return;
    var ids = cy.nodes(":selected").map(function (node) {
      return node.id();
    });
    callbacks.onSelectionChange(ids);
  }

  function mount(element, graphData, callbacks) {
    if (!window.cytoscape) {
      element.textContent = "Cytoscape is not loaded.";
      return null;
    }
    element.innerHTML = "";
    var elements = buildElements(graphData || {});
    element.__mandoforgeGraphDebug = {
      phase: "mount",
      payloadNodes: Array.isArray((graphData || {}).nodes) ? graphData.nodes.length : -1,
      payloadEdges: Array.isArray((graphData || {}).edges) ? graphData.edges.length : -1,
      elementCount: elements.length
    };
    var cy = window.cytoscape({
      container: element,
      elements: elements,
      style: makeStyles(),
      layout: makeLayout(graphData || {}),
      minZoom: 0.22,
      maxZoom: 2.2,
      boxSelectionEnabled: true,
      selectionType: "additive"
    });

    cy.on("tap", "node", function (event) {
      var id = event.target.id();
      if (callbacks && typeof callbacks.onSelect === "function") {
        callbacks.onSelect(id);
      }
      focusNeighborhood(cy, id);
    });

    cy.on("tap", "edge", function (event) {
      var edge = event.target;
      edge.select();
      focusNeighborhood(cy, edge.source().id());
      if (callbacks && typeof callbacks.onSelect === "function") {
        callbacks.onSelect(edge.source().id());
      }
    });

    cy.on("select unselect", "node", function () {
      emitSelection(cy, callbacks);
    });

    applySelection(cy, graphData || {});
    focusNeighborhood(cy, graphData && graphData.selected_node_id);
    element.__mandoforgeCy = cy;
    return cy;
  }

  function update(instance, graphData) {
    if (!instance) return null;
    var elements = buildElements(graphData || {});
    var container = instance.container();
    if (container) {
      container.__mandoforgeGraphDebug = {
        phase: "update",
        payloadNodes: Array.isArray((graphData || {}).nodes) ? graphData.nodes.length : -1,
        payloadEdges: Array.isArray((graphData || {}).edges) ? graphData.edges.length : -1,
        elementCount: elements.length
      };
    }
    instance.batch(function () {
      instance.elements().remove();
      instance.add(elements);
    });
    instance.layout(makeLayout(graphData || {})).run();
    applySelection(instance, graphData || {});
    focusNeighborhood(instance, graphData && graphData.selected_node_id);
    return instance;
  }

  function fit(instance) {
    if (instance) instance.fit(undefined, 36);
  }

  function resetLayout(instance, graphData) {
    if (!instance) return;
    instance.layout(makeLayout(graphData || {})).run();
    instance.fit(undefined, 36);
  }

  function selectNodes(instance, selectedNodeId, selectedIds) {
    if (!instance) return;
    var ids = Array.isArray(selectedIds) ? selectedIds : [];
    instance.batch(function () {
      instance.nodes().unselect();
      ids.forEach(function (id) {
        instance.getElementById(id).select();
      });
    });
    focusNeighborhood(instance, selectedNodeId);
  }

  function destroy(instance) {
    if (instance) instance.destroy();
  }

  window.MandoForgeOntologyGraph = {
    mount: mount,
    update: update,
    fit: fit,
    resetLayout: resetLayout,
    selectNodes: selectNodes,
    destroy: destroy
  };
})();
