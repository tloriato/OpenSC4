# dispatch: open questions

2026-10-25 (re-dispatchview8): DP10 is answered and archived. The vehicle view and the dispatch
tool are specified as whole owners in dispatch-vehicle-view.md, on top of dispatch §4.1.3.
DP11, DP12 and DP13 remain answered and archived.
<!-- Append new questions at the end; IDs continue docs/specs/QUESTIONS.md. -->

## DP-SE36-1 - immediate flag at the manager's untracking sites

Open; services-exact36; 2026-10-28. Dispatch 4.1.3 defines the two paths of
UntrackOccupant(handle, immediate), but 3's RemoveOccupant and 4.4's end/revoke
paths do not specify the immediate argument supplied by the manager. Please
state that Boolean for each site. The new retained-view host uses false with
a SPEC-GAP comment pending this contract. No measured difference has yet been
attributed to that choice; this is an interface-use question, not an exactness
claim or a request for implementation excerpts.
