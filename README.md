# gugu

Interaction Nets based language. No variables. No stack. No execution order.

```gu
# @ADD and Nat rules come from std/nat.gu (auto-loaded)
@GEN : @ADD /lft=1 /rgt=(>out) -> 1
# -> 2
```

---

gugu programs are webs. Execution is web rewriting.
Two atoms meet at their fuse — a rule fires — the web changes.
That's it. That's everything.

## Model

| concept | meaning                                       |
|---------|-----------------------------------------------|
| `atom`  | the smallest unit. `@ADD` `@ZERO` `@ERA`      |
| `bond`  | connection between atoms                      |
| `fuse`  | fuse. where sparks happen           |
| `arm`   | arms. `/lft` `/rgt` `/hi`           |
| `spark` | two fuses connected — waiting to bloom        |
| `bloom` | a rule fires. the web rewrites. one step.     |
| `web`   | the whole web. a program is an initial web. |
| `slag`  | no sparks remain. computation is done.        |

```
web reached slag after 42 blooms
```
## Properties

**no variables** — `/a` does not exist at runtime.
it is a parser label. when the web is built, only bonds remain.
names dissolve into structure.

**parallelism is free** — confluence theorem (Lafont, 1990).
any order of blooms yields the same slag.
independent sparks can bloom simultaneously.
no threads. no mutexes. no async.

**memory is free** — bond anything to `@ERA`.
the entire structure it holds dissolves, recursively.
no malloc. no free. no GC.

## This is the foundation of everything

gugu has no if, no for, no while, no functions.
there are only two built-in atoms.

```gu
# ERA — erases. dissolves everything bonded to it.
rule @ERA >< _ :
  all arms -- @ERA

# DUP — duplicates. splits one atom into two.
rule @DUP >< @ZERO :
  ~/c1 -- @ZERO
  ~/c2 -- @ZERO

rule @DUP >< @BIT1 :
  @DUP /c1=>>d1 /c2=>>d2 -> ~/hi
  ~/c1 -- @BIT1 /hi=>>d1
  ~/c2 -- @BIT1 /hi=>>d2
```

from ERA and DUP, all conditionals, loops, and recursion are derived.
control flow is not a keyword. it is a consequence of rules.

## Theory

Based on Interaction Nets — Yves Lafont, POPL 1990.

- **confluence**: bloom order does not affect the final slag
- **strong normalization**: terminating programs always reach slag
- **linearity**: every bond connects exactly two ports

## License

MIT