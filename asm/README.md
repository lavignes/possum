# possum-asm

PossumASM is a nice z80 macro assembler.

## Features

* Powerful expression evaluator using signed 32-bit integers.
* Supports most undocumented opcodes.
* Syntax for declaring `@struct`s and `@enum`s.
* Pretty nice error messages.
* Global and local labels.

## Command Line

```
USAGE:
    possum-asm [OPTIONS] <FILE>

ARGS:
    <FILE>    Path to input assembly file

OPTIONS:
    -h, --help                 Print help information
    -I, --include <INCLUDE>    Paths to search for included files (Repeatable)
    -o, --output <OUTPUT>      Path to output binary file (Default: stdout)
    -V, --version              Print version information
```

## Syntax

### Comments

Comments start with a semicolon (`;`) and continue to the end of the line.

```
    ; this is a comment
```

### Instructions

Instructions and all built-in keywords are case-insensitive:

```
    LD A, B
    ld a, b
```

### Whitespace

For the most part, whitespace has zero semantic impact on PossumASM.
Most programs need zero line breaks if you dont want them.

This is valid:

```
add a, b add b, c ret call $1234 @echo "Hello World"
```

As is this:

```
subroutine1 inc a ret subroutine2 call subroutine3 subroutine3 dec a ret
```

### Encoding

PossumASM uses utf-8 encoding. Therefore you can happily put emojis
in your code with no trouble:

```
@echo "Howdy Cowboy! 🤠"
```

### Numbers

PossumASM supports binary, decimal, and hexadecimal numeric literals.

#### Binary

Binary numbers start with a percent-sign / modulus (`%`) followed by a
string of `0`s and `1`s:

```
    @echo %10100011
```

#### Decimal

Decimal numbers are any string of the numbers `0` through `9`:

```
    @echo 42
```

### Hexadecimal

Hexadecimal numbers start with a dollar-sign (`$`) followed by a string of the
numbers `0` through `9` and the letters `a` through `f`.

```
    @echo $cafe
```

### Strings

Strings are enclosed by double-quotation marks (`"`):

```
    @echo "Hello World"
```

Most standard C-language string escape sequences are supported:

```
    @echo "A new line\n"

    @echo "Multi\
line"

    @echo "A quote: \"lorem ipsum\""
```

However, hexadecimal escapes are prefixed with a dollar-sign (`$`) rather than the
C-style `x` character:

```
    @echo "A new line\$0a"
```

### Characters

An in C, character literals can encode 1-4 bytes. The syntax for them is identical
to strings other than the fact that they are enclosed by single-quotation marks
(`'`):

```
    @echo 'q'
```

They also support escape sequences:

```
    @echo '\n'
```

And as mentioned, can be used to describe a **little-endian** multi-byte sequence:

```
    @echo 'win' ; Represents the number $006e6977
```

### Labels

PossumASM supports 3 kinds of labels:

#### Global

The first are "global" or *regular* labels. These act like
normal labels in most assemblers.

```
subroutine:
    nop
    ret
```

#### Local

Local labels are syntactic sugar for labels that are "owned" by
or *local* to a global label. They are declared with a starting dot (`.`):

```
subroutine:
.loop:
    call do_thing
    jp z, .loop
    ret

do_thing:
.loop:
    dec a
    jp nz, .loop
    ret
```

Here the local label `.loop` can be declared and referenced twice with no
problems since the global labels `subroutine` and `do_thing` create a
context by which the two usages can be disambiguated.

#### Direct

Use *direct* labels to directly reference local labels "owned" by other
global labels. You write them with the format `<global_label>.<local_label>`:

```
jp subroutine.loop
jp do_thing.loop
```

#### Optional Colon

Also note that the use of a colon (`:`) when defining a label
is **optional**.

### Expressions

PossumASM has a full-featured expression parser that supports every C operator
except for comma (`,`). There are some important things to keep in mind:

* Operator precedence matches that of C.
* All operands are evaluated as 32-bit **signed** values.
* All operations wrap-around. That is, adding `1` to the number `$ffffffff` results in `0`.
* Logical operators work like C. `0` is treated as `false`, anything else is `true`.
* The shift operators (`<<` and `>>`) are signed shifts. To perform an unsigned shift
  use the unsigned shift left (`<:`) and unsigned shift right (`:>`) operators.

#### Lazy Evaluation

Internally, PossumASM is a two-pass assembler. Though unlike many simple
assemblers, PossumASM supports deferring most expression evaluations until
they are absolutely needed. This allows chains of forward references
of arbitrary depth to be evaluated.

#### Expression Examples

```
    @echo 1 + 2 * 3
```

```
    @echo ~42
```

```
    @def truthy, %0110
    @def falsey, 0

    @echo truthy || falsey
```

```
    @def test, 40
    @def even,  0
    @def odd, 1

    @echo ((test % 2) == 0) ? even : odd
```

## Directives

### `@echo`

The `@echo` directive is useful for logging and debugging. It takes
a single string or expression argument and prints it on `stderr`.

```
    @echo "Hello World"
    @echo 1 + 2 + 3
```

### `@assert`

`@assert` takes an expression argument followed by an optional
string message. If the expression evaluates to `0` then the assembler
is terminated with the string printed as the failure reason.

```
    @def VALUE, 2
    @assert VALUE % 0 == 0, "VALUE must be even"
```

### `@die`

Terminates assembly with an error message. The string or expression
argument will be printed as the failure reason.

```
    @die "Failed"
```

### `@def`

Use `@def` to *define* labels equal to expressions. The first argument
is the label and the second is any expression.

#### Global

```
    @def Global, 1234 + 5678 

    @echo Global
```

#### Local

```
    Global:
        @def .value, $42
        
        @echo .value 
        @echo Global.value
```

### `@include`

`@include` will in-line an assembly file into the file currently
being read by the assembler. Use this to break up your code into multiple
files for ease of organization.

The argument is a string representing a file path that is relative to the
current file being read. If no such file can be found the search paths
of the assembler are checked in order.

```
@include "file.asm"      ; A local file or one in the search path
@include "../above.asm"  ; A file in the parent directory
```

### `@db` and `@dw`

The `@db` and `@dw` directives are used to define strings of bytes
or 2-byte words respectively.

The `@db` directive accepts one or more string or expression arguments
and the `@dw` takes one or more expression arguments.

The data is placed directly into the output at the point they are
declared.

```
    @db "Hello", "World", '\n'

    @dw $1234, $5678, $cafe, $beef, 'ow'
```

### `@ds`

`@ds` stands for "define space". It is used to add repeated byte values
to the output. It takes a size expression, followed by an optional value.
If no value is provided, then the value is assumed to be `0`.

For example, this places four bytes into the output with the value of `$42`:

```
    @def SIZE, 1 + 3

    @ds SIZE, $42
```

A common use for `@ds` is to pad the output with zeros to ensure data
or subroutines are properly placed at specific locations:

```
@assert sub2 == $0100, "sub2 must start at $0100"

sub1:
    call sub2
    ret

@ds $0100 - @here 
sub2:
    ret
```

In this example, we add padding between `sub1` and `sub2` to ensure that
`sub2` starts at `$0100`.

You'll also notice the use of an assertion here. Not only that, but
the assertion is written before `sub2` is defined. This is fine in PossumASM
as assertions can be deferred until linking the final binary. If the assertion
were to be placed on or after the line defining `sub2` then it could fail
before link-time.

### `@here`

The `@here` directive is an expression that evaluates to the value
of the internal program counter of the assembler. As your assembly
is assembled, the program counter increments relative to the size
of your code and data.

For example, when assemble this trivial program:

```
    @echo @here
    nop
    @echo @here
```

The assmbler will output this on `stderr`:

```
0
1
```

At the start of assembly, the internal program counter has a value
of `0`.

The size of the `nop` instruction is 1 byte. So the program counter
is incremented by `1`.

This directive has many uses, but a common one is recording the
length of string constants:

```
HELLO:
    @db "Hello World"

    @def .len, @here - HELLO

@echo HELLO.len
```

Here we begin by defining a global label: `HELLO`.
It points to the location of a string of bytes: `"Hello World"`.

The `@db` directive will increment the program counter by the total
number of bytes, in this case `11`.

Then we define the local label: `.len`. Since it is after the `HELLO`
label, the fully-qualified *direct* label will be `HELLO.len`.

The value of `HELLO.len` is set to `@here - HELLO`. Let's say for the
sake of example that `HELLO` was defined at `$0100`. That means that
`@here` will be equal to `$010B` (`$0100 + 11`). Therefore, the
expression will evaluate to `$010b - $0100` or `11`-- the length of
the string.

While assembling, `stderr` will display `11`.

### `@org`

The `@org` directive is a sibling to `@here`. Instead of returning
the value of the program counter, it instead sets the value.

The main use of `@org` is for defining relocatable code.

**The `@org` directive does not add any padding to your output.
Use `@ds` to advance the program counter and add filler bytes.**

### `@struct` and `@ends`

The `@struct` directive provides syntactic sugar for declaring
structured data. You use structs to define offsets for *fields* in
your data structures as well as to keep track of the total size of
a structure.

The argument to `@struct` is the struct's name. A minimal struct
with zero fields would look like this:

```
@struct MyStruct
@ends
```

Note the use of `@ends` to end the struct definition. **This is required.** 

Inside of the struct definition are the list of field names and expressions
that evaluate to their sizes in **bytes**:

```
@struct MyStruct
    tag 1
    name 16
    name_len 1
@ends
```

This defines the following labels:

* `MyStruct.tag` equal to `0`
* `MyStruct.name` equal to `1`
* `MyStruct.name_len` equal to `17`
* `MyStruct` equal to `18`

The first 3 labels represent the offsets from the beginning of a `MyStruct`
used to find the field's data. The struct name itself is defined as the total
size of the struct in bytes.

Suppose for example you want to create a `MyStruct` and set the field
`name_len` to the value `8`:

```
the_struct:
    @ds MyStruct

ld ix, the_struct
ld (ix+MyStruct.name_len), 8
```

### `@enum` and `@ende`

The `@enum` directive allows one to create simple enumerated labels:

```
@enum MyEnum
    zero
    one
    two
    tree
@ende
```

This defines the following labels:

* `MyEnum.zero` equal to `0`
* `MyEnum.one` equal to `1`
* `MyEnum.two` equal to `2`
* `MyEnum.three` equal to `3`
* `MyEnum` equal to `4`

Each direct label represents the index zero-based index of itself variant
in the enumeration. The label of the enumeration itself is a simple count
of all variants.
