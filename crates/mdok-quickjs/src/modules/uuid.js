/*
 * uuid capability shim (pinned registry).
 *
 * The corpus (and older Postman collections) call the module directly:
 *   var uuid = require("uuid"); uuid()
 * The modern `uuid` package exports {v1,v3,v4,v5,...} instead. This shim
 * provides both shapes, generating v4 UUIDs from Math.random (the compat
 * profile allows QuickJS Math; no host crypto is exposed).
 */
(function () {
  'use strict';
  function hex(n) { return n.toString(16).padStart(2, '0'); }
  function rnd() { return Math.floor(Math.random() * 256); }
  function v4() {
    var bytes = [];
    for (var i = 0; i < 16; i++) bytes.push(rnd());
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant
    var out = '';
    for (var j = 0; j < 16; j++) {
      out += hex(bytes[j]);
      if (j === 3 || j === 5 || j === 7 || j === 9) out += '-';
    }
    return out;
  }
  function v1() {
    // Pseudo-v1: same shape, random node/clock (not time-based; no clock exposed).
    var bytes = [];
    for (var i = 0; i < 16; i++) bytes.push(rnd());
    bytes[6] = (bytes[6] & 0x0f) | 0x10; // version 1
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    var out = '';
    for (var j = 0; j < 16; j++) {
      out += hex(bytes[j]);
      if (j === 3 || j === 5 || j === 7 || j === 9) out += '-';
    }
    return out;
  }
  function uuid() { return v4(); }
  uuid.v1 = v1;
  uuid.v3 = function () { return v4(); };
  uuid.v4 = v4;
  uuid.v5 = function () { return v4(); };
  uuid.NIL = '00000000-0000-0000-0000-000000000000';
  uuid.parse = function (s) {
    return String(s).replace(/-/g, '').match(/../g).map(function (b) {
      return parseInt(b, 16);
    });
  };
  uuid.stringify = function (bytes) {
    var out = '';
    for (var j = 0; j < 16; j++) {
      out += hex(bytes[j] & 0xff);
      if (j === 3 || j === 5 || j === 7 || j === 9) out += '-';
    }
    return out;
  };
  if (typeof module !== 'undefined' && module && module.exports) {
    module.exports = uuid;
  }
  return uuid;
})();
