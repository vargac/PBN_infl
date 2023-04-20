class ClickableTable {
    constructor(table, selectColor, selectCallback) {
        this.table = table;
        this.tBody = table.tBodies[0];

        this._selectColor = selectColor;
        this._selectCallback = selectCallback;

        this._selected = null;
        this._locked = false;

        this._setCallbacks();
    }

    reset() {
        this.tBody.remove();
        this.tBody = document.createElement('tbody');
        this.table.append(this.tBody);
        this._setCallbacks();
    }

    get lock() { return this._locked; }
    set lock(value) { this._locked = value; }

    get selected() { return this._selected; }

    _setCallbacks() {
        this.tBody.onclick = this._onclick.bind(this);
    }

    _onclick(event) {
        let row = event.target.closest('tr');
        if (!row)
            return;

        if (this._selected)
            this._selected.style.backgroundColor = '';

        this._selected = row;
        this._selected.style.backgroundColor = this._selectColor;
        this._selected.style.color = 'black';

        this._selectCallback(this._selected);
    }
}
