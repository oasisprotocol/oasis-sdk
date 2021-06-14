let messageFrame = null;
const responseHandlers = {};
let nextId = 0;
window.addEventListener('message', (e) => {
    if (e.origin !== 'chrome-extension://joglombbipnjdfbkimehokiomlbhcobn') return;
    switch (e.data.type) {
        case 'oasis-xu-ready':
            {
                messageFrame = e.source;

                const id = nextId++;
                responseHandlers[id] = (/** @type {MessageEvent<any>} */ e) => {
                    console.log('got public key', e.data.public_key);
                };
                messageFrame.postMessage({
                    type: 'context-signer-public',
                    id,
                }, 'chrome-extension://joglombbipnjdfbkimehokiomlbhcobn');
                break;
            }
        case 'oasis-xu-response':
            {
                const id = +e.data.id;
                if (!(id in responseHandlers)) return;
                const h = responseHandlers[id];
                delete responseHandlers[id];
                h(e);
                break;
            }
    }
});
