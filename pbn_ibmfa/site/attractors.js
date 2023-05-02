class Attractors {
    constructor(field, selectedVarField, onSelected) {
        this._field = field;
        this._selectedVarField = selectedVarField;
        this._map = new Map();
        this._var_names = undefined;
        this.table = new ClickableTable(
            field.getElementsByTagName('table')[0],
            'lightgreen',
            (row) => { onSelected(row.cells[0].innerHTML); }
        );
        this._count = 0;
    }

    set var_names(value) { this._var_names = value; }

    reset() {
        this.table.reset();
        this._count = 0;
    }

    add(colors, state) {
        let row = document.createElement('tr');
        let state_html = '';
        for (let bit of state) {
            state_html += `<span class='var_bit'>${bit}</span>`;
        }
        row.innerHTML = `
            <td>${this._count}</td>
            <td>${colors}</td>
            <td>${state_html}</td>
            <td>?</td>
            <td>?</td>
        `;
        this._count++;
        this.table.tBody.appendChild(row);
        this._make_var_bits(this.table.tBody.lastElementChild);
    }

    _make_var_bits(element) {
        let var_bits = element.getElementsByClassName('var_bit');
        const colors = { '-': 'gray', '0': 'red', '1': 'green' };
        let field = this._selectedVarField;
        for (let i = 0; i < var_bits.length; i++) {
            let var_name = this._var_names[i];
            var_bits[i].onmouseover = function() {
                let color = colors[this.innerHTML];
                this.style.backgroundColor = color;
                field.innerHTML = var_name
                field.style.color = color;
            };
            var_bits[i].onmouseout = function() {
                this.style.background = null;
                field.innerHTML = "";
            };
        }
    }

    _get_row(id) { return this.table.tBody.rows[id]; }

    get_attractor(id) {
        let cells = this._get_row(id).cells;
        return {
            colors: +cells[1].innerHTML,
            state: cells[2].innerHTML,
            entropy: cells[3].innerHTML
        };
    }

    set_entropy(id, entropy) {
        this._get_row(id).cells[3].innerHTML = entropy;
    }

    set_driver_set(id, driver_set_str) {
        let driver_set = new Map();
        for (let fix of driver_set_str.slice(2, -2).split(' ')) {
            let [name, value] = fix.split('=');
            driver_set.set(name, value);
        }

        let dset = this._var_names
            .map(name => driver_set.has(name) ? driver_set.get(name) : '-')
            .join('');

        let dset_html = '';
        for (let bit of dset) {
            dset_html += `<span class='var_bit'>${bit}</span>`;
        }

        let element = this._get_row(id).cells[4];
        element.innerHTML = dset_html;
        this._make_var_bits(element);
    }

    get length() { return this._count; }
    get selected() { return +this.table.selected.cells[0].innerHTML; }
    get hidden() { return this._field.hidden; }
    set hidden(value) { this._field.hidden = value; }
}
