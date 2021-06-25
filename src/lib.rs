/*!
This crate provides a high performance, low level 2D rasterization library
with support for rendering paths of various styles into alpha or subpixel
masks.

Broadly speaking, support is provided for the following:
- 256x anti-aliased rasterization (8-bit alpha or 32-bit RGBA subpixel alpha)
- Pixel perfect hit testing with customizable coverage threshold
- Non-zero and even-odd fills
- Stroking with the standard set of joins and caps
    (separate start and end caps are possible)
- Numerically stable dashing for smooth dash offset animation
- Vertex traversal for marker placement
- Stepped distance traversal for animation or text-on-path support
- Abstract representation of path data that imposes no policy on storage

While this crate is general purpose, in the interest of interoperability and
familiarity, the feature set was chosen specifically to accommodate the
requirements of the
[SVG path specification](https://www.w3.org/TR/SVG/paths.html).

Furthermore, the rasterized masks are nearly identical to those generated by
Skia (sans slight AA differences) and as such, should yield images that are
equivalent to those produced by modern web browsers.

# Rendering

Due to the large configuration space for styling and rendering paths, the
builder pattern is used pervasively. The [Mask](struct.Mask.html) struct is the
builder used for rasterization. For example, to render a simple triangle
into a 64x64 8-bit alpha mask:

```rust
use zeno::{Mask, PathData};

// The target buffer that will contain the mask
let mut mask = [0u8; 64 * 64];

// Create a new mask with some path data
Mask::new("M 8,56 32,8 56,56 Z")
    // Choose an explicit size for the target
    .size(64, 64)
    // Finally, render the path into the target
    .render_into(&mut mask, None);
```

Note that, in this case, the path itself is supplied as a string in SVG path
data format. This crate provides several different kinds of path data by
default along with support for custom implementations. See the
[PathData](trait.PathData.html) trait for more detail.

The previous example did not provide a style, so a non-zero
[Fill](enum.Fill.html) was chosen by default. Let's render the same path with
a 4 pixel wide stroke and a round line join:

```rust
use zeno::{Join, Mask, PathData, Stroke};

let mut mask = [0u8; 64 * 64];

Mask::new("M 8,56 32,8 56,56 Z")
    .size(64, 64)
    .style(Stroke::new(4.0).join(Join::Round))
    .render_into(&mut mask, None);
```

Or to make it a bit more dashing:

```rust
use zeno::{Cap, Join, Mask, PathData, Stroke};

let mut mask = [0u8; 64 * 64];

Mask::new("M 8,56 32,8 56,56 Z")
    .style(
        Stroke::new(4.0)
            .join(Join::Round)
            .cap(Cap::Round)
            // dash accepts a slice of dash lengths and an initial dash offset
            .dash(&[10.0, 12.0, 0.0], 0.0),
    )
    .size(64, 64)
    .render_into(&mut mask, None);
```

See the [Stroke](struct.Stroke.html) builder struct for all available options.

So far, we've generated our masks into fixed buffers with explicit sizes. It is
often the case that it is preferred to ignore all empty space and render a path
into a tightly bound mask of dynamic size. This can be done by eliding the call
for the size method:

```rust
use zeno::{Mask, PathData};

// Dynamic buffer that will contain the mask
let mut mask = Vec::new();

let placement = Mask::new("M 8,56 32,8 56,56 Z")
    // Insert an inspect call here to access the computed dimensions
    .inspect(|format, width, height| {
        // Make sure our buffer is the correct size
        mask.resize(format.buffer_size(width, height), 0);
    })
    .render_into(&mut mask, None);
```

The call to size has been replaced with a call to inspect which injects a
closure into the call chain giving us the opportunity to extend our buffer to
the appropriate size. Note also that the render method has a return value that
has been captured here. This [Placement](struct.Placement.html) struct
describes the dimensions of the resulting mask along with an offset that should
be applied during composition to compensate for the removal of any empty space.

Finally, it is possible to render without a target buffer, in which case the
rasterizer will allocate and return a new `Vec<u8>` containing the mask:

```rust
use zeno::{Mask, PathData};

// mask is a Vec<u8>
let (mask, placement) = Mask::new("M 8,56 32,8 56,56 Z")
    // Calling render() instead of render_into() will allocate a buffer
    // for you that is returned along with the placement
    .render();
```

Both [Mask](struct.Mask.html) and [Stroke](struct.Stroke.html) offer large
sets of options for fine-grained control of styling and rasterization including
offsets, scaling, transformations, formats, coordinate spaces and more. See
their respective documentation for more detail.

# Hit testing

Hit testing is the process of determining if a point is within the region that
would be painted by the path. A typical use case is to determine if a user's
cursor is hovering over a particular path. The process generally follows the
same form as rendering:

```rust
use zeno::{HitTest, PathData};

// A 20x10 region with the right half covered by the path
let hit_test = HitTest::new("M10,0 10,10 20,10 20,0 Z");

assert_eq!(hit_test.test([15, 5]), true);
assert_eq!(hit_test.test([5, 5]), false);
```

Due to the fact that paths are anti-aliased, the hit test builder offers a
threshold option that determines how much "coverage" is required for a hit test
to pass at a particular point.

```rust
use zeno::{HitTest, PathData};

let mut hit_test = HitTest::new("M2.5,0 2.5,2 5,2 5,0 Z");

// Require full coverage for a successful hit test
hit_test.threshold(255);
assert_eq!(hit_test.test([2, 0]), false);

// Succeed for any non-zero coverage
hit_test.threshold(0);
assert_eq!(hit_test.test([2, 0]), true);
```

See the [HitTest](struct.HitTest.html) type for more detail.

# Path building

While SVG paths are a reasonable choice for static storage, there sometimes
arise cases where paths must be built dynamically at runtime:

```rust
use zeno::{Command, Mask, PathBuilder, PathData};

// Create a vector to store the path commands
let mut path: Vec<Command> = Vec::new();

// Construct the path with chained method calls
path.move_to([8, 56]).line_to([32, 8]).line_to([56, 56]).close();

// Ensure it is equal to the equivalent SVG path
assert!((&path).commands().eq("M 8,56 32,8 56,56 Z".commands()));

// &Vec<Command> is also valid path data
Mask::new(&path).render(); // ...
```

Here, a vector of [Command](enum.Command.html)s is used to store the path data
and the [PathBuilder](trait.PathBuilder.html) trait provides the extension
methods necessary for building a path.

Beyond the four basic path commands, the path builder trait also provides
arcs (and position relative versions of all previous commands) along with
rectangles, round rectangles, ellipses and circles:

```rust
use zeno::{Angle, ArcSize, ArcSweep, Command, PathBuilder, PathData};

let mut path: Vec<Command> = Vec::new();

path.move_to([1, 2]).rel_arc_to(
    8.0,
    4.0,
    Angle::from_degrees(30.0),
    ArcSize::Small,
    ArcSweep::Positive,
    [10, 4],
);

assert!((&path).commands().eq("M1,2 a8,4,30,0,1,10,4".commands()));
```

Along with incremental building of paths, path builder can also be used as a
"sink" for capturing the result of the application of a style and transform
to some path data. For example, it is possible to store the output of a stroke
style to avoid the cost of stroke evaluation for future rendering or hit test
operations with the use of the [apply](fn.apply.html) function:

```rust
use zeno::{apply, Cap, Command, PathBuilder, PathData, Stroke};

let mut stroke: Vec<Command> = Vec::new();

apply("L10,0", Stroke::new(4.0).cap(Cap::Round), None, &mut stroke);
```

[PathBuilder](struct.PathBuilder.html) is only implemented for `Vec<Command>`
by default, but custom implementations are possible to support capturing
and building paths into other data structures.

# Traversal

Path traversal involves incremental evaluation of a path by some metric. This
crate currently provides two methods of traversal.

The [Vertices](struct.Vertices.html) iterator yields a variant of the
[Vertex](enum.Vertex.html) enum at the beginning and end of each subpath and
between each path command. Each variant provides all the geometric
information necessary to place SVG style markers.

The [Walk](struct.Walk.html) type is an iterator-like type that allows for
stepping along the path by arbitrary distances. Each step yields the position
on the path at the next distance along with a vector describing the
left-ward direction from the path at that point. This is useful for animating
objects along a path, or for rendering text attached to a path.

# Transient memory allocations

The algorithms in this crate make a concerted effort to avoid dynamic
allocations where possible, but paths of significant size or complexity
may cause spills into temporary heap memory. Specifically, stroke evaluation
and rasterization may cause heap allocations.

To amortize the cost of these, the appropriately named
[Scratch](struct.Scratch.html) struct is available. This type contains internal
heap allocated storage and provides replacement methods for functions that may
allocate. In addition, the
[Mask::with_scratch](struct.Mask.html#method.with_scratch) and
[HitTest::with_scratch](struct.HitTest.html#method.with_scratch)
constructors are provided which take a scratch instance as an argument and
redirect all transient allocations to the reusable storage.
 */

mod command;
mod geometry;
#[cfg(feature = "eval")]
mod hit_test;
#[cfg(feature = "eval")]
mod mask;
mod path_builder;
mod path_data;
#[cfg(feature = "eval")]
mod raster;
#[cfg(feature = "eval")]
mod scratch;
mod segment;
#[cfg(feature = "eval")]
mod stroke;
mod style;
mod svg_parser;
#[cfg(feature = "eval")]
mod traversal;

pub use command::{Command, Verb};
pub use geometry::{Angle, Bounds, Origin, Placement, Point, Transform, Vector};
#[cfg(feature = "eval")]
pub use hit_test::HitTest;
#[cfg(feature = "eval")]
pub use mask::{Format, Mask};
pub use path_builder::{ArcSize, ArcSweep, PathBuilder};
pub use path_data::{length, PathData};
#[cfg(feature = "eval")]
pub use path_data::{apply, bounds};
#[cfg(feature = "eval")]
pub use scratch::Scratch;
pub use style::*;
pub use svg_parser::validate_svg;
#[cfg(feature = "eval")]
pub use traversal::{Vertex, Vertices, Walk};

// Prep for no_std support when core supports FP intrinsics.
mod lib {
    pub use std::vec::Vec;
}
