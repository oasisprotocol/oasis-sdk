// @ts-check
global.XMLHttpRequest = require('xhr2');
const oasis = require('./..');

import('../playground/src/startPlayground.mjs').then(async ({startPlayground}) => {
    console.log(await startPlayground(oasis));
});
