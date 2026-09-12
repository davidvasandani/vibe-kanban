# Mobile Toolbar Layout Contract

For a mobile workspace navbar:

- the workspace toolbar region grows into remaining inline space, may shrink
  below its content width, and owns horizontal overflow;
- the visible workspace-tab group fills at least the toolbar region;
- visible workspace tabs share surplus width and retain a usable minimum width;
- the trailing status/action region does not shrink;
- all tools remain on one line and reachable;
- active tabs expose `aria-pressed="true"` and keep their existing visual
  indicator; and
- safe-area/window-control padding remains applied by the navbar row.

For a mobile project navbar and for every desktop navbar, the prior layout
contract is unchanged.
