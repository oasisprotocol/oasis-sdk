// @ts-check
global.XMLHttpRequest = require('xhr2');
if (typeof crypto === 'undefined') {
    throw 'Upgrade to Node.js@>=19 or Node.js@>=16 with --experimental-global-webcrypto.'
}

const oasis = require('./..');

import('../playground/src/startPlayground.mjs').then(async ({startPlayground}) => {
    await startPlayground(oasis);
});
