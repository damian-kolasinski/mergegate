These golden images capture the same native selectable reader used by the app in light, dark,
and accessibility text sizes, including unwrapped, horizontally scrolling source.
DiffGoldenTests and HighlightGoldenTests attach each actual render to the xcresult. To update an intentional rendering change,
export that test's attachments, inspect them, and replace the matching PNG in this directory.
The renderer uses scale 1 with fixed font/appearance traits for stable comparison on iOS 26.2.
