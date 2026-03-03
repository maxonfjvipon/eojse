const REMOVE_UNNECESSARY = true
const USE_CACHE = true
const COPY_ON_APPLICATION = false
const USE_D_SCOPES = true
const REMOVE_D_MARKED = true
const WITH_SCOPE_DEFAULT = false
const HIDE_XI = false

const FORMATION = "FRM", DISPATCH = "DSP", APPLICATION = "APP", COPY = "CPY", SET = "SET"

const PHI = 'φ'
const DELTA = 'Δ'
const RHO = 'ρ'
const LAMBDA = 'λ'

const stack = {}

const print_object = (index) => {
  const obj = stack[index]
  let res
  const form = (o) => {
    let r = '{'
    Object.keys(o).forEach((at, idx) => {
      if (idx > 0) {
        r += ', '
      }
      r += `'${at}':`
      if (at === LAMBDA || o[at].value == null) {
        r += `'${o[at].value}'`
      } else if (at === DELTA) {
        r += `[${o[at].value}]`
      } else if (o[at].xi == null && o[at].cache == null) {
        r += o[at].value
      } else if (o[at].value === o[at].cache) {
        r += `${o[at].value}!`
      } else {
        r += '{' + [o[at].value, HIDE_XI ? '*' : o[at].xi, o[at].cache].join(', ') + '}'
      }
    })
    r += '}'
    return r
  }
  switch (obj.type) {
    case FORMATION:
      res = [
        `${index}: ${form(obj.target)}`,
        // `${idx}: ${JSON.stringify(obj.target)}`,
        Object.hasOwn(obj.target, DELTA) ? ' (DATA ' + bytesOf.bytes(obj.target[DELTA].value).verbose() + ')' : '',
        !!obj.stay ? ' (STAY)' : '',
        Object.hasOwn(obj, 'from_atom') ? ' (FROM ATOM)' : '',
      ].join('')
      break
    case DISPATCH:
      res = [
        `${index}: `,
        `${obj.target}.${obj.attr}`,
      ].join('')
      break
    case APPLICATION:
      res = [
        `${index}: `,
        `${obj.target}(${obj.attr}: ${obj.value})`,
      ].join('')
      break
  }
  return res + ' // ' + obj.name
}

const print_stack = () => {
  Object.keys(stack).map(Number).forEach((idx) => {
    console.log(print_object(idx))
  })
}

const hexToInt = function (bytes) {
  let byte
  return bytes.map((hex) => {
    if (typeof hex === 'number' && Number.isInteger(hex)) {
      byte = hex
    } else if (hex.length === 4 && hex.indexOf('0x') === 0) {
      byte = parseInt(hex, 16)
    } else {
      throw new Error(
        `Wrong format of element ${hex} in byte array ${bytes}\nShould be either integer of hexadecimal starting with '0x'`
      )
    }
    return byte
  })
}

const bytesOf = {
  bytes: function (bytes) {
    if (!Array.isArray(bytes)) {
      throw new Error(`Can't take byte array bytes from non byte array (${bytes})`)
    }
    return conversion(hexToInt(bytes))
  },
  number: function (num) {
    if (typeof num !== 'number') {
      throw new Error(`Can't take number bytes from not a number (${num})`)
    }
    const buffer = new ArrayBuffer(8)
    const view = new DataView(buffer)
    view.setFloat64(0, num)
    return conversion(Array.from(new Int8Array(buffer)))
  },
  string: function (str) {
    if (typeof str !== 'string') {
      throw new Error(`Can't take string bytes from non string (${str})`)
    }
    return conversion(Array.from(Buffer.from(str, 'utf-8')))
  },
  bool: function (bool) {
    if (typeof bool !== 'boolean') {
      throw new Error(`Can't take boolean bytes from non boolean (${bool})`)
    }
    return conversion(bool ? [1] : [0])
  },
}

const types = {
  SHORT: 'short',
  INT: 'int',
  LONG: 'long',
  NUMBER: 'number',
  STRING: 'string',
  BOOL: 'bool',
  BYTES: 'bytes'
}
const { LONG, INT, SHORT, NUMBER } = types

const conversion = function (bytes) {
  return {
    asBytes: function () {
      return bytes
    },
    asNumber: function (type = NUMBER) {
      let res
      if (type === NUMBER && bytes.length === 8) {
        res = new DataView(new Int8Array(bytes).buffer).getFloat64(0)
      } else if (type === LONG && bytes.length === 8) {
        res = new DataView(new Int8Array(bytes).buffer).getBigInt64(0)
      } else if (type === INT && bytes.length === 4) {
        res = BigInt(new DataView(new Int8Array(bytes, 4).buffer).getInt32(0))
      } else if (type === SHORT && bytes.length === 2) {
        res = BigInt(new DataView(new Int8Array(bytes, 6).buffer).getInt16(0))
      } else {
        throw new Error(`Unsupported conversion to '${type}' from ${bytes}`)
      }
      return res
    },
    asString: function () {
      return Buffer.from(bytes).toString('utf-8')
    },
    asBool: function () {
      if (bytes.length !== 1) {
        throw new Error(`Byte array must be 1 byte long to convert to bool (${bytes})`)
      }
      return bytes[0] !== 0
    },
    verbose: function () {
      let str
      if (bytes.length === 0) {
        str = '--'
      } else if (bytes.length === 1) {
        if (bytes[0] === 1) {
          str = 'true'
        } else if (bytes[0] === 0) {
          str = 'false'
        } else {
          str = `[${bytes[0]}]`
        }
      } else if (bytes.length === 2) {
        str = `[${this.asBytes()}] = ${this.asNumber(SHORT)}, or "${this.asString()}"`
      } else if (bytes.length === 4) {
        str = `[${this.asBytes()}] = ${this.asNumber(INT)}, or "${this.asString()}"`
      } else if (bytes.length === 8) {
        // str = `[${this.asBytes()}] = ${this.asNumber()}, or ${this.asNumber(LONG)}, or "${this.asString()}"`
        str = `${this.asNumber()}`
      } else {
        str = `[${this.asBytes()}] = "${this.asString()}"`
      }
      return str
    }
  }
}

module.exports = {
  print_object,
  print_stack,
  bytesOf,
  REMOVE_UNNECESSARY,
  USE_CACHE,
  COPY_ON_APPLICATION,
  USE_D_SCOPES,
  REMOVE_D_MARKED,
  WITH_SCOPE_DEFAULT,
  HIDE_XI,
  FORMATION,
  APPLICATION,
  DISPATCH,
  COPY,
  SET,
  stack,
  PHI,
  DELTA,
  RHO,
  LAMBDA
}
