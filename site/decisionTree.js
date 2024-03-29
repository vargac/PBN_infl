function htmlTitle(html) {
    let container = document.createElement('div');
    container.style.fontSize = '10px';
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
        let partition = new Map();
        let colors = +attractor.colors;

        this.parse(tree_str.split(' '), colors, nodes, edges, partition);
        for (let node of nodes) {
            let dset_colors = partition.get(node.label);
            if (dset_colors) {
                node.title.innerHTML += `<br>Total colors: ${dset_colors}`;
            }
        }

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
                    nodeSpacing: 150,
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

        let ent = 0.0;
        for (let count of partition.values()) {
            ent += count * Math.log2(count);
        }
        ent = - (ent / colors - Math.log2(colors));
        return ent < 0 ? 0 : ent;
    }

    parse(tree, colors, nodes, edges, partition) {
        if (tree[0] == '[') {
            let i = 1;
            let title = '';
            let driver_set = new Map();
            for (; tree[i] != ']'; i++) {
                let [name, value] = tree[i].split('=');
                driver_set.set(name, value);
                let color = value == '1' ? 'green' : 'red';
                title += `<span style="color: ${color}">${name}</span>`;
                title += i % 2 == 1 ? ' ' : '<br>';
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

            if (!partition.has(label)) {
                partition.set(label, +colors);
            } else {
                partition.set(label, partition.get(label) + +colors);
            }
            return i + 1;
        }

        let decision_label = '', decision_title = '';
        let parameters = new Set();
        for (let decision of tree[0].split(';')) {
            let regs_index = decision.indexOf('(');
            if (regs_index != -1) {
                let regs = decision.slice(regs_index + 1, -1);
                let title = decision.slice(0, regs_index) + '(';
                parameters.add(decision.slice(
                    decision.startsWith('!') ? 1 : 0, regs_index));
                for (let reg of regs.split(',')) {
                    let color = reg[0] == '1' ? 'green' : 'red';
                    let name = reg.slice(1);
                    title += `<span style="color: ${color}">${name}</span> `;
                }
                decision_title += title.slice(0, -1) + ')';
            } else {
                parameters.add(decision);
                decision_title += decision;
            }
            decision_title += '<br>';
        }
        decision_title = htmlTitle(decision_title);
        decision_label = Array.from(parameters).join('\n');

        let colors_false = tree[1], colors_true = tree[2];
        let read = 3;
        read += this.parse(
            tree.slice(read, tree.length), colors_false,
            nodes, edges, partition
        );
        let left = nodes[nodes.length - 1].id;
        read += this.parse(
            tree.slice(read, tree.length), colors_true,
            nodes, edges, partition
        );
        let right = nodes[nodes.length - 1].id;
        let current = right + 1;

        nodes.push({id: current, label: decision_label, title: decision_title});
        edges.push({from: current, to: left,
                    color: 'red', label: colors_false, title: colors_false});
        edges.push({from: current, to: right,
                    color: 'green', label: colors_true, title: colors_true});
        return read;
    }
}
