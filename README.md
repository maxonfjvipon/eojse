# Flat EOLANG runtime emulator

This is the JS emulator of [EOLANG](https://github.com/objectionary/eo) runtime
which tries to map the object-oriented paradigm to imperative operations.

## Why it exists

OOP is slow, pure OOP is even slower. Can it be faster? We're sure it can.
The only way to make pure OOP language as fast as procedural one - impose
restrictions on the language, which allow to present complicated manipulations
with objects as set of imperative instructions and effectively store them in
linear memory.

We believe that EOLANG has such restrictions. On one hand, the language is
quite simple and does not contain huge amount features which most famous OOP
languages have (classes, types, mutability, etc.), on other hand, it's still
pure object-oriented.

## The main idea and key concepts

### Memory

We assume that we work with memory as linear sequence of segments (cells)
where we're laying out the object one after another.
For now, we don't divide the memory into stack and heap.

In this emulator the role of memory is performed by `memory` variable.
It's a JS object where each key is an address of the object and value is object
itself. Keys are started from `0` and incremented for each created object for
convenience.

If we say `push` - it means we place new object to the memory after the last
one, if we say `pop` - it means we remove the last object from the memory,
similar to stack

### Objects

EO objects stored in memory can be one of three types:

- Formation
- Application
- Dispatch

In this emulator each of the EO object is JS object of the following
structure:

```javascript
Object = {
  name: String             // name of the object
  type: String             // type of the object
  target: Formation|Number // if 'type' is "formation" the 'target' is Formation
                           // if 'type' is "application" or "dispatch" the target is Number -
                           // address of the target object in memory
  attr: String|Number|null // name or index of the attribute for Application or Dispatch
                           // null if type is "formation"
  value: Number|null       // adress of the object to set for Application
                           // null if type is not "application"
}

// Formation is actually regular JS object where keys are EO object attributes
// and values are Attributes
Formation = {
  "attr1": Attribute
  "attr2": Attribute
  ...
}

Attribute = {
  value: Number // address to the object in memory
  xi: Number|null // context
  cache: Number|null // cache
}
```

Consider this EO program:

```
[] > foo
  $.x Q.y > @
  [] > x
```

The `memory` for that program looks like this (fields with `null` are ignored):

```javascript
{
  0: {name: "Q", type: "formation", target: {"foo": {value: 1}}},
  1: {name: "foo", type: "formation", target: {"@": {value: 2}, "x": {value: 5}}},
  2: {name: "$.x Q.y", type: "application", target: 3, attr: 0, value: 4},
  3: {name: "$.x", type: "dispatch", target: -1, attr: "x"},
  4: {name: "Q.y", type: "dispatch", target: 0, attr: "y"},
  5: {name: "x", type: "formation", target: {}}
}
```

As you see, each object has rigid structure so we may definitely say what size
in bytes it has. It may be useful for further implementations.

### Dataization

The objects by themselves are not interested until they contain some data
which we want to get as a result of program execution. Dataization is the core
process in EOLANG which allows to extract the data from the program. Data in
EOLANG is a sequence of bytes. Bytes can be attached only to the special `Δ`
attribute (or asset) of Formation. This is how data is stored in current
implementation:

```javascript
42: {name: "?", type: "formation", "target": {"Δ": {value: ['0x00', '0x01', ..., '0x10']}}}
```

As you see the data can be extracted only from Formation which contains `Δ`
asset.

1. If `Δ` asset is absent, first we check if Formation contains `φ` attribute
   and if so, we push to the memory new object which is dispatch of `φ`
   attribute from current formation.
2. If `φ` is also absent, we check if Formation contains `λ` asset and if so,
   we execute atom which is attached to this `λ` asset.
3. If `λ` is also absent - dataization fails.

In both successful cases we get new object pushed in `memory`. Then we try to
dataize it again. But it's not guaranteed that we get Formation returned from
atom. But how to extract data from Application or dispatch? We should bring
them to Formations first, and they try to find `Δ` asset again.

### Morphing

Morphing is the process of converting any object to Formation. For more
details read this (link to phi paper). Morphing of Formation returns Formation
itself. Morphing of Dispatch and Application in most cases pushes new objects
to `memory` because it requires nested morphing operations.

### Copying

The only way a new object can be pushed to the `memory` is via `copy` operation.
The only Formation can be the argument of copy operation. The `copy` operation
is executed only during morphing of Dispatch. The `copy` operation in current
implementation has one significant difference from `φ` calculus and java
implementation: we don't make a deep copy of an object.

Consider this example in `φ` calculus:

```
{[ x -> [ y -> [ D> 01- ] ], @ -> Q.x.y ]}
```

During dataization of this program we go through such steps:

```
   {[ x -> [ y -> [ D> 01- ], z -> [] ], @ -> Q.x.y ]} =>
=> {[ x -> [ y -> [ D> 01- ], z -> [] ], @ -> [ x -> [ y -> [ D> 01- ], z -> [] ] ].x.y ]} =>
=> {[ x -> [ y -> [ D> 01- ], z -> [] ], @ -> [ y -> [ D> 01- ], ^ -> ... ].y ]} =>
=> {[ x -> [ y -> [ D> 01- ], z -> [] ], @ -> [ D> 01-, ^ -> ... ] ]} =>
=> 01-
```

As you maybe see, when we copy the object attached to `x` attribute, we copy
all the nested attributes: `y` and `z`. The same happens in the java
implementation.

In current implementation attributes in formations are attached via links
(addresses in `memory`), so we are able not to copy entire objects.
_In future versions we may think about even lighter coping._

Consider this example in current implementation:

```javascript
// [] > foo
//   [] > x
{
  0: {name: "Q", type: "formation", target: {"foo": {value: 1}}},
  1: {name: "foo", type: "formation", target: {"x": {value: 2}}},
  2: {name: "x", type: "formation", target: {}}
}
```

If we need to copy `foo` object, we don't need to copy the object `x`:

```javascript
{
  0: {name: "Q", type: "formation", target: {"foo": {value: 1}}},
  1: {name: "foo", type: "formation", target: {"x": {value: 2}}},
  2: {name: "x", type: "formation", target: {}}
  3: {name: "foo'", type: "formation", target: {"x": {value: 2}}},
}
```

### Clearing

During program dataization new objects are pushed to the `memory`. At some
point of time they become unnecessary for future calculations, and we should
remove them.

The main question - when and where we can stop the calculations and try to
clear unnecessary objects. (**linear objects arrangement???**)

#### Situation 1.

When the global dataization process reaches the Formation (let's call it
`O1`), it checks if the `O1` has `Δ` asset. If it does not - the `φ` attribute
is taken from the `O1`. This `01.φ` dispatch leads to creating some amount of
new objects pushed to `memory`. At some moment we reach some Formation `O2`
which is actually attached to the `φ` attribute of `O1`. Then dataization
process tries to take `Δ` asset from `O1`:

Consider this example before dataization of `O1`

```javascript
...
10: {name: 'O1', type: 'formation', target: {'φ': {value: 8, }}} // O1
```

This is how `memory` looks like after taking `φ` attribute from `O1`:

```javascript
...
10: {name: 'O1', type: 'formation', target: {'φ': {value: 8, cache: 15 }}} // O1
...
13: {name: 'O3', type: 'formation', target: {...}}
...
15: {name: 'O2', type: 'formation', target: {'φ': {value: 5}, 'ρ': {value: 13}}} // O2
```

As you see taking `φ` attribute from `O1` led to pushing 5 objects to the
`memory`. Objects `10`, `13` and `15` are connected with each other so we can't
delete them. Other objects can be painlessly removed. Next time, when we
take next `φ` attribute from `O2` and get new Formation `O3`, we get next set
of objects that can be deleted, maybe including the objects from previous set:
`10`, `13` or `15`.

So, each time when we see the situation when we need go though `φ`:
1. we remember the address of the object, we need to take `φ` attribute from,
   e.g. `10` for `O1`.
2. when we get the `O2`, we remember its address, e.g. `15`.
3. we're going though memory from start to the end and check what objects are
   not connected with result `O2`.
4. we mark such object as `to-be-removed`
5. remove them somehow. In current implementation we just do
   `delete memory[index]`. In more or less real example with real memory we
   should do via memory shifting which is more complicated task

#### Situation 2

**The second case is not tested and implemented yet.**

It's similar to the first one, but we remember addresses when we need to go
through `λ`.

#### Situation 3

**The third case is not tested and implemented yet.**

There are other dataization processes besides the global one - inside atoms.
This situation is more complex because scope where we can mark and delete
object is limited.

Consider this example:

```javascript
10: {name: 'O1', type: 'formation', target: {'λ': {value: 'L_atom_foo'}}}
```

We're trying to dataize `O1`. We start executing atom `L_atom_foo`. During its
execution new objects are pushed to the `memory`. And at some moment atom
starts inner dataization of some object, e.g. `O2`:

```javascript
10: {name: 'O1', type: 'formation', target: {'λ': {value: 'L_atom_foo'}}} // here atom starts
...
15: {name: 'O2', type: 'formation', target: {...}} // atoms dataizes this object
```

Similar to case 1 or 2, during the dataization we can go though `φ` or `λ`
attributes for a several times. We also remembers the addresses of specified
objects, but marking and removing a bit complicated, because other objects
outside of atom scope may refer to these `to-be-removed` objects.

```javascript
7: {name: 'O0', type: 'formation', target: {'x': {value: 4, cache: 17}}}
...
10: {name: 'O1', type: 'formation', target: {'λ': {value: 'L_atom_foo'}}} // here atom starts
...
15: {name: 'O2', type: 'formation', target: {'φ': {value: 5, cache: 18}}} // atoms dataizes this object
...
17: {name: 'O4', type: 'formation', target: {...}}
18: {name: 'O5', type: 'formation', target: {...}}
```

Here `O2.φ` refers to `O5`, object `O4` is not connected with `O5` or `O2` but
it's connected with `O0`, which is outside of scope of dataization inside
atom (which is from `15` to `18`). So object `17` can't also be removed.

#### Situation 4

At some moment of time dataization process (global or not) reaches the
formation with `Δ` asset and data will be extracted. At this point of time we
also get some amount of unnecessary objects in `memory` which can be removed.
If dataization is global - we can just remove all the objects added to
`memory` during program execution, because end of dataization of the program
means the end of the program. If dataization is not global, but inside atom,
we can't just delete every object that was creating during the process. We can
remove only objects which are not connected with objects outside the scope of
current dataization process. _For now such removing is implemented only for
global dataization._

#### Situation 5

_We believe there are much more cases in which we can stop the main dataization
process and remove unnecessary object. Just need to find them_

### How to play

Test programs are placed inside `test-resources` directory.
To disable or enable a specific program, you can comment/uncomment it
in `test/test_programs.js` file.

Then run `npm test`. It compiles EO program into JS, creates `temp` directory
with your programs inside and run them (takes around 11 second per each
program)

To see the full log and `memory` status for a specific program, e.g. `fibo`
you can do:

```bash
node temp/fibo/.eoc/program.js
```

The main `runtime` code is in `resources/program.js`.

To add new `eo-runtime` objects you should extend `resources/runtime.xsl`.

### How to contribute

You need `node` installed on your computer.

Submit a PR with your changes. To avoid frustration please run `npm test`
before sending. Make sure all the tests pass.
