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

module.exports = bytesOf
