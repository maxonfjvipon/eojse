const path = require('path')
const fs = require('fs')
const parser = require('xml2json')
const {execSync} = require("child_process")
const assert = require('assert')
const yaml = require('js-yaml')

const temp = path.resolve(__dirname, '..', 'temp')
const resources = path.resolve(__dirname, '..', 'test-resources')
const saxon = path.resolve(__dirname, '..', 'SaxonHE12-9J', 'saxon-he-12.9.jar')
const runner = path.resolve(__dirname, '..', 'resources', 'program.js')
const deps = ['bytes.js'].map((pth) => path.resolve(__dirname, '..', 'resources', pth))
const eo_parser = '0.59.4'

const xsls = [
  'remove-xi.xsl',
  'simple-runtime.xsl',
  'runtime.xsl',
  'application.xsl',
  'formation.xsl',
  'void.xsl',
  'dispatch.xsl',
  'noise.xsl',
  'ids.xsl',
  'cache.xsl',
  'flat-apps-forms.xsl',
  'flat-disps.xsl',
  'to-js.xsl'
]

const run = (cmd, cwd = null) => {
  return execSync(cmd, {stdio: 'pipe', cwd: cwd || process.cwd()})
}

const rmDirRecursive = (dir) => {
  try {
    if (!fs.existsSync(dir)) {
      return;
    }
    const entries = fs.readdirSync(dir);
    for (const entry of entries) {
      const full = path.join(dir, entry);
      const stat = fs.statSync(full);
      if (stat.isDirectory()) {
        rmDirRecursive(full);
      } else {
        fs.unlinkSync(full);
      }
    }
    fs.rmdirSync(dir);
  } catch (error) {
    throw error;
  }
}

if (fs.existsSync(temp)) {
  rmDirRecursive(temp)
}

fs.mkdirSync(temp, {recursive: true})

const programs = [
  'simple',
  'foo',
  'eleven',
  'rec',
  "fibo",
  "self-ref",
  "fibo_minus"
]

describe('run programs', function () {
  programs.forEach((name) => {
    it(`should run '${name}' program`, function (done) {
      this.timeout(0)
      const dir = path.resolve(temp, name)
      if (fs.existsSync(dir)) {
        rmDirRecursive(dir)
      }
      fs.mkdirSync(dir, {recursive: true})

      const source = path.resolve(resources, `${name}.yaml`)
      const eo = path.resolve(dir, 'program.eo')
      const xmir = path.resolve(dir, '.eoc', '1-parse', 'program.xmir')
      const output = path.resolve(dir, '.eoc', '2-result', 'program.xmir')
      const prog = path.resolve(dir, '.eoc', 'program.js')

      const {program, expected} = yaml.load(fs.readFileSync(source, 'utf8'))

      fs.writeFileSync(path.resolve(dir, 'program.eo'), program)

      try {
        // parse EO to XMIR
        run(`eoc parse --clean --easy --parser=${eo_parser} ${eo}`, dir)

        let input = xmir
        let out
        xsls.forEach((xsl, idx) => {
          const pth = path.resolve(__dirname, '../resources', xsl)
          out = output.replace('.xmir', '-' + idx + '.xmir')
          run(`java -jar ${saxon} -s:${input} -xsl:${pth} -o:${out}`)
          input = out
          run(`xcop --fix ${out}`)
        })
        const xml = fs.readFileSync(out)
        const js = parser.toJson(xml.toString(), {object: true})['object']['$t'].split('\n').map(txt => txt.trim()).join('\n')

        // copy and paste
        const content = fs.readFileSync(runner).toString().replace('// OBJECTS', js)
        fs.writeFileSync(prog, content)

        deps.forEach((dep) => {
          fs.writeFileSync(path.resolve(dir, '.eoc', path.basename(dep)), fs.readFileSync(dep, {encoding: 'utf-8'}))
        })

        const stdout = run(`node ${prog}`).toString()
        assert.ok(stdout.includes(`data: ${expected}`))
        done()
      } catch (error) {
        console.log(error)
        throw error
      }
    })
  })
})
