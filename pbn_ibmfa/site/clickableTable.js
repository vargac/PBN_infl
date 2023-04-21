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

    resize() {
        let header = this.table.tHead.rows[0].cells;
        let body = this.table.tBodies[0].rows[0].cells;
        for (let i = 0; i < header.length; i++) {
            let w = Math.max(header[i].clientWidth, body[i].clientWidth);
            let header_style = window.getComputedStyle(header[i]);
            let body_style = window.getComputedStyle(body[i]);
            let header_padding =
                `${header_style.getPropertyValue('padding-left')}
                + ${header_style.getPropertyValue('padding-right')}`;
            let body_padding =
                `${body_style.getPropertyValue('padding-left')}
                + ${body_style.getPropertyValue('padding-right')}`;

            header[i].style.width = `calc(${w}px - (${header_padding}))`;
            body[i].style.width = `calc(${w}px - (${body_padding}))`;
        }
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

        if (this._selected) {
            this._selected.style.backgroundColor = null;
            this._selected.style.color = null;
        }

        this._selected = row;
        this._selected.style.backgroundColor = this._selectColor;
        this._selected.style.color = 'black';

        this._selectCallback(this._selected);
    }
}
