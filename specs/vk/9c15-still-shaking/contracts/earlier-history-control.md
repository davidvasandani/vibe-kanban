# Contract: Earlier-History Control

Given the earlier-history presentation props:

- idle and loading states render one shared geometry-owning container;
- both state layers occupy the same grid cell and participate in intrinsic
  sizing, so the container reserves the larger state's block size in both;
- loading state remains announced visually and to assistive technology;
- idle invokes the supplied load callback;
- error state exposes retry and an error status;
- absent state is not rendered by the parent;
- the component does not read or write scroll position.

The parent continues to own pagination and semantic anchor correction.
