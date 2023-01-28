// @ts-check

import * as oasis from './../..';
import {startPlayground} from './startPlayground.mjs';

export const playground = startPlayground(oasis);

playground.catch((e) => {
    console.error(e);
});
