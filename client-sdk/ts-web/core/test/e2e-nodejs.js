// @ts-check
const nock = require('nock')
nock.recorder.rec({
  output_objects: true
})
global.XMLHttpRequest = require('xhr2');
const oasis = require('./..');

import('../playground/src/startPlayground.mjs').then(async ({startPlayground}) => {
    await startPlayground(oasis);
});
