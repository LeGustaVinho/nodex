// Classic JavaScript: CommonJS, prototypes, hoisting and IIFEs.
"use strict";

var EventEmitter = require("events");
var helpers = require("./modules/defaults.js");

function Counter(start) {
  if (!(this instanceof Counter)) {
    return new Counter(start);
  }
  this.value = start || 0;
}

Counter.prototype.increment = function () {
  this.value++;
  return this;
};

var instance = (function () {
  var count = 0;
  return {
    next: function () {
      return ++count;
    },
  };
})();

function sum() {
  var total = 0;
  for (var i = 0; i < arguments.length; i++) {
    total += arguments[i];
  }
  return total;
}

exports.extra = helpers;
module.exports = { Counter: Counter, sum: sum, instance: instance, EventEmitter: EventEmitter };
