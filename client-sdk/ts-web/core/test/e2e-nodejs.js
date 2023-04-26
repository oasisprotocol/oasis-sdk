// @ts-check
const nock = require('nock')
nock.recorder.rec({
  output_objects: true
})
global.XMLHttpRequest = require('xhr2');
const oasis = require('./..');

import('../playground/src/startPlayground.mjs').then(({startPlayground}) => {
  setTimeout(async () => {
    await startPlayground(oasis);
  }, 2*60*1000)
});
