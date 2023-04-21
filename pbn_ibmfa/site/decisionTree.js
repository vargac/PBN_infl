function htmlTitle(html) {
    let container = document.createElement('div');
    container.innerHTML = html;
    return container;
}

class DecisionTree {
    constructor(field, layoutButton) {
        this._field = field;
        this._layoutButton = layoutButton;
        this._var_names = null;
    }

    set var_names(value) { this._var_names = value; }

    reset() {
        this._field.innerHTML = "";
    }

    show(tree_str) {
        let nodes = [];
        let edges = [];
        this.parse(tree_str.split(' '), nodes, edges);

        let data = {
            nodes: new vis.DataSet(nodes),
            edges: new vis.DataSet(edges),
        };

        let options = {
            edges: {
                arrows: 'to',
                physics: false,
                smooth: true,
            },
            layout: {
                hierarchical: {
                    enabled: true,
                    levelSeparation: 50,
                    sortMethod: 'directed',
                    shakeTowards: 'leaves',
                },
            },
            physics: {
                enabled: false,
            },
        };

        let network = new vis.Network(this._field, data, options);
        // enable moving the nodes
        network.setOptions({layout: { hierarchical: false}});

        this._layoutButton.onclick = function(event) {
            network.setOptions(options);
            network.setOptions({layout: { hierarchical: false}});
        };
    }

    parse(tree, nodes, edges) {
        if (tree[0] == '[') {
            let i = 1;
            let title = '';
            let driver_set = new Map();
            for (; tree[i] != ']'; i++) {
                let [name, value] = tree[i].split('=');
                driver_set.set(name, value);
                let color = value == '1' ? 'green' : 'red';
                title += `<span style="color: ${color}">${name}</span> `;
            }
            let label = this._var_names
                .map(name => driver_set.has(name) ? driver_set.get(name) : '-')
                .join('');

            let next_id = nodes.length ? (nodes[nodes.length - 1].id + 1) : 1;
            nodes.push({
                id: next_id,
                title: htmlTitle(title),
                label: label,
            });
            return i + 1;
        }

        let decision = tree[0];
        let read = 1;
        read += this.parse(tree.slice(read, tree.length), nodes, edges);
        let left = nodes[nodes.length - 1].id;
        read += this.parse(tree.slice(read, tree.length), nodes, edges);
        let right = nodes[nodes.length - 1].id;
        let current = right + 1;
        nodes.push({id: current, label: decision});
        edges.push({from: current, to: left, color: 'red', label: '0'});
        edges.push({from: current, to: right, color: 'green', label: '1'});
        return read;
    }
}
