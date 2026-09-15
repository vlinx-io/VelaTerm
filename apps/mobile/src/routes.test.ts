import { test } from 'node:test';
import assert from 'node:assert/strict';
import { routeFromHash, editHref, copyHref } from './routes.ts';
test('new and edit connection links restore route state',()=>{
 assert.deepEqual(routeFromHash('#/connections/new?mode=ssh'),{page:'edit',mode:'ssh'});
 assert.deepEqual(routeFromHash(editHref('Gerät/a?')), {page:'edit',mode:'url',id:'Gerät/a?'});
 assert.deepEqual(routeFromHash('#/'),{page:'list'});
});

test('scan entry restores URL form without putting the scanned address in navigation',()=>{
 assert.deepEqual(routeFromHash('#/connections/new?mode=url&scan=1'),{page:'edit',mode:'url',scan:true});
 assert.deepEqual(routeFromHash('#/connections/new?mode=ssh&scan=1'),{page:'edit',mode:'ssh'});
 assert.deepEqual(routeFromHash('#/connections/new?mode=url'),{page:'edit',mode:'url'});
});

test('account and Remote pages restore from independent URLs', () => {
  assert.deepEqual(routeFromHash('#/account'), {page:'account'});
  assert.deepEqual(routeFromHash('#/remote'), {page:'remote'});
});

test('copy links restore a separate draft without treating the source as an edit', () => {
  assert.deepEqual(routeFromHash(copyHref('Gerät/a?')), {page:'edit',mode:'url',copyFromId:'Gerät/a?'});
  assert.notEqual(copyHref('source'), editHref('source'));
});
