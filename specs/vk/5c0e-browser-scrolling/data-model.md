# Data and contract
No durable model changes. Scroll is a new snake_case BrowserAction variant: {type:"scroll",x:i32,y:i32,delta_x:i32,delta_y:i32}. Positive deltas scroll right/down. Existing WS command ids, generation checks, responses and errors are unchanged. Gesture state lives only in the panel and is discarded on session, connection, control-generation or ownership changes.
