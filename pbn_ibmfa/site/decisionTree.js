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

    show(tree_str, attractor) {
        let nodes = [];
        let edges = [];
        let entropy = { value: 0.0 };
        let colors = +attractor.colors;
        this.parse(tree_str.split(' '), colors, nodes, edges, entropy);

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

        let e = - (entropy.value / colors - Math.log2(colors));
        return e < 0 ? 0 : e;
    }

    parse(tree, colors, nodes, edges, entropy) {
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
            entropy.value += colors * Math.log2(colors);
            return i + 1;
        }

        let decision = tree[0];
        let colors_false = tree[1], colors_true = tree[2];
        let read = 3;
        read += this.parse(
            tree.slice(read, tree.length), colors_false, nodes, edges, entropy);
        let left = nodes[nodes.length - 1].id;
        read += this.parse(
            tree.slice(read, tree.length), colors_true, nodes, edges, entropy);
        let right = nodes[nodes.length - 1].id;
        let current = right + 1;
        nodes.push({id: current, label: decision});
        edges.push({from: current, to: left,
                    color: 'red', label: colors_false, title: colors_false});
        edges.push({from: current, to: right,
                    color: 'green', label: colors_true, title: colors_true});
        return read;
    }
}
