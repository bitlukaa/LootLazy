# Home preview 3.11.4

Local verification record before publication. Stable publication was subsequently authorized by the user.

Home uses the existing status poll and in-memory account snapshots. Opening it does not start the account engine or sign in saved accounts. It shows Game Time, Game Variety, Quests, Steam Idle and grouped account sessions, with precise session controls and pending counts. Resume is shown only for supported paused state; stopped sessions are not silently restarted. Existing quest pause support remains available, while Variety presents Start/Stop only.

Pointer ordering prevents native image dragging and retains row nodes during incoming status updates. Drag grips support immediate mouse/touch dragging; row long presses and Alt+Arrow remain available. The renderer tests hold a drag across a status interval and force a render before dropping.

Steam artwork has multiple trusted CDN sources. A failed-primary/successful-alternate fixture verifies decoding and placeholder removal. The public Battlefield 6 header (app 2807960) returned HTTP 200 during inspection; not every third-party title is guaranteed to have artwork.

Verification: frontend unit/renderer suites and isolated Tauri/WebView2 session recovery and Home controls. No real account sign-in, achievement writes or player reports are performed by these checks. The preview contains separately copied encrypted saved account files for user-driven trials; installed data is preserved and these files are excluded from Git.
