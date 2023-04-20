'use strict';

window.onload = function() {
    // HTML entities
    let startButton      = document.getElementById('startButton');
    let connectButton    = document.getElementById('connectButton');
    let pbnFile          = document.getElementById('pbnFile');
    let statusShow       = document.getElementById('statusShow');
    let connectionPort   = document.getElementById('connectionPort');
    let infoField        = document.getElementById('info');
    let varsCnt          = document.getElementById('varsCnt');
    let colorsCnt        = document.getElementById('colorsCnt');

    function showConnected() {
        statusShow.className = "connected";
        statusShow.innerHTML = "Ready";
        connectButton.hidden = true;
        connectionPort.setAttribute('readonly', true);
        pbnFile.parentNode.hidden = false;
    }
    function showNotConnected() {
        statusShow.className = "notConnected";
        statusShow.innerHTML = "Not connected";
        connectButton.hidden = false;
        connectionPort.removeAttribute('readonly');
        pbnFile.parentNode.hidden = true;
    }
    function showComputing() {
        statusShow.className = "computing";
        statusShow.innerHTML = "Computing";
    }


    let ws = null;
    connectionPort.value = 5678;
    let file = null;
    let attractors = new Attractors(
        document.getElementById('attractors'),
        document.getElementById('selectedVar'),
        onAttractorSelect
    );
    showNotConnected();

    function connect(port) {
        ws = new WebSocket(`ws://127.0.0.1:${port}/`);
        // TODO show message on error
        ws.onopen = onopen;
        ws.onclose = onclose;
    }

    function onopen(event) {
        showConnected();
        pbnFile.value = "";
    }

    function onclose(event) {
        showNotConnected();
        startButton.hidden = true;
        infoField.hidden = true;
        attractors.hidden = true;
        attractors.reset();
    }


    startButton.onclick = function() {
        if (!pbnFile.value) {
            alert("No file selected.");
            return;
        }

        startButton.hidden = true;
        attractors.reset();
        attractors.hidden = false;

        ws.send('START');
        showComputing();
        ws.onmessage = function(event) {
            let data = event.data.split(' ');
            console.log(data);
            for (let i = 0; i < data.length / 2; i++) {
                attractors.add(data[2 * i], data[2 * i + 1]);
            }
            showConnected();
        };
    };

    connectButton.onclick = function() {
        let value = connectionPort.value;
        if (/[0-9]/.test(value) && +value <= 65535 && 1024 <= +value) {
            connect(value);
        } else {
            alert('Port has to be a number in range [1024, 65535]');
        }
    };

    pbnFile.onchange = function(e) {
        if (file) {
            if (!confirm("The current model will be overwritten. Proceed?")) {
                return;
            }
        }
        file = e.target.files[0];
        ws.send(file);
        ws.onmessage = function(event) {
            console.log(event.data);
            let [cmd] = event.data.split(' ', 1);
            switch (cmd) {
                case 'ERR':
                    alert(event.data.slice(cmd.length));
                    pbnFile.value = "";
                    file = null;
                    break;
                case 'OK':
                    let [_, colors, ...var_names] = event.data.split(' ');
                    attractors.var_names = var_names;
                    attractors.hidden = true;
                    infoField.hidden = false;
                    varsCnt.innerHTML = var_names.length;
                    colorsCnt.innerHTML = colors;
                    startButton.hidden = false;
                    break;
                default:
                    alert("Unexpected error.");
                    break;
            }
        };
    };

    function onAttractorSelect(id) {
        attractors.table.lock = true;
        ws.send(`TREE ${id}`);
    }
};
