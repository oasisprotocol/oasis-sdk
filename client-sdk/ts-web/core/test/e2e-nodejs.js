// @ts-check
const nock = require('nock');
nock.recorder.rec({
  output_objects: true
});
const xhr2 = require('xhr2');
const http = require('http');
global.XMLHttpRequest = xhr2;
// Enable Nagle's algorithm
xhr2.nodejsSet({ httpAgent: new http.Agent({ noDelay: false }) });

const oasis = require('./..');

import('../playground/src/startPlayground.mjs').then(async ({startPlayground}) => {
  console.log(await startPlayground(oasis));
});
