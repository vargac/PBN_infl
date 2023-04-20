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
    }

    add(colors, state) {
        let row = document.createElement('tr');
        let state_html = '';
        for (let bit of state) {
            state_html += `<span class='var_bit'>${bit}</span>`;
        }
        row.innerHTML =
            `<td>${this._count}</td><td>${colors}</td><td>${state_html}</td>`;
        this._count++;
        this.table.tBody.appendChild(row);
        let var_bits =
            this.table.tBody.lastElementChild.getElementsByClassName('var_bit');
        let field = this._selectedVarField;
        for (let i = 0; i < var_bits.length; i++) {
            let var_name = this._var_names[i];
            var_bits[i].onmouseover = function() {
                let color = this.innerHTML == '1' ? 'green' : 'red';
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

    get length() { return this._count; }
    get selected() { return this.table.selected.cells[0].innerHTML; }
    get hidden() { return this._field.hidden; }
    set hidden(value) { this._field.hidden = value; }
}
