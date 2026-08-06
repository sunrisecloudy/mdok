/*
 * querystring capability shim (pinned registry).
 *
 * Node's `querystring` module is not available in QuickJS; this shim covers
 * the documented subset used by Postman scripts: parse/stringify/encode/
 * decode (application/x-www-form-urlencoded semantics).
 */
(function () {
  'use strict';
  function encodeComponent(s) {
    return encodeURIComponent(String(s))
      .replace(/%20/g, '+')
      .replace(/[!'()*]/g, function (c) {
        return '%' + c.charCodeAt(0).toString(16).toUpperCase();
      });
  }
  function decodeComponent(s) {
    try {
      return decodeURIComponent(String(s).replace(/\+/g, ' '));
    } catch (e) {
      return String(s);
    }
  }
  function stringify(obj, sep, eq, options) {
    sep = sep || '&';
    eq = eq || '=';
    var out = [];
    function add(key, value) {
      var k = encodeComponent(key);
      if (value === null || value === undefined) {
        out.push(k);
      } else if (Array.isArray(value)) {
        for (var i = 0; i < value.length; i++) {
          out.push(k + eq + encodeComponent(value[i]));
        }
      } else if (typeof value === 'object' && options && options.encodeValuesOnly) {
        out.push(k + eq + encodeComponent(JSON.stringify(value)));
      } else {
        out.push(k + eq + encodeComponent(value));
      }
    }
    for (var key in obj) {
      if (Object.prototype.hasOwnProperty.call(obj, key)) add(key, obj[key]);
    }
    return out.join(sep);
  }
  function parse(str, sep, eq, options) {
    sep = sep || '&';
    eq = eq || '=';
    var out = {};
    if (typeof str !== 'string' || str.length === 0) return out;
    var parts = String(str).split(sep);
    for (var i = 0; i < parts.length; i++) {
      var pair = parts[i];
      if (!pair) continue;
      var idx = pair.indexOf(eq);
      var key = idx >= 0 ? decodeComponent(pair.slice(0, idx)) : decodeComponent(pair);
      var value = idx >= 0 ? decodeComponent(pair.slice(idx + 1)) : '';
      if (Object.prototype.hasOwnProperty.call(out, key)) {
        if (Array.isArray(out[key])) out[key].push(value);
        else out[key] = [out[key], value];
      } else {
        out[key] = value;
      }
    }
    return out;
  }
  var api = {
    parse: parse,
    stringify: stringify,
    encode: encodeComponent,
    unescape: decodeComponent,
    decode: parse,
    escape: encodeComponent,
  };
  if (typeof module !== 'undefined' && module && module.exports) {
    module.exports = api;
  }
  return api;
})();
