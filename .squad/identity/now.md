# What Tracey's team is focused on now

**Phase:** Phase 3  US1 COMPLETE (pending manual checkpoint)
**Current focus:** Phase 3 checkpoint + Phase 4 prep

## Phase 3  ALL TASKS COMPLETE  (pending build verification)
- T018/T019: TDD gate (Shaw)  43 failing tests committed 
- T020: timer_start 
- T021: timer_stop + timer_get_active 
- T022: time_entry_list 
- T023: time_entry_create_manual + overlap detection 
- T024: time_entry_continue 
- T025: time_entry_autocomplete + is_orphaned 
- T025a: E2E orphaned suggestion test 
- T026: tracey://timer-tick emitter 
- T027: TimerStateService.cs 
- T028: QuickEntryBar.razor 
- T029: TimeEntryList.razor 
- T029a: E2E scroll preservation test 
- T030: Dashboard.razor 
- T030a: time_entry_update 
- T030b: Inline edit (auto-save on blur) 
- T030c: E2E inline edit tests 

## Phase 3 Checkpoint (manual  Vincent runs this)
Terminal 1: dotnet watch run --project src/Tracey.App --urls http://localhost:5000
Terminal 2 (from src-tauri/): cargo tauri dev

Verify:
- Quick-entry bar focused on launch
- Type description + Enter  timer starts
- Timer counting up (live tick)
- Ctrl+Space stops timer
- Entry appears in list
- Continue button works

## Next: Phase 4  Idle Detection (US2)
When Vincent gives the go, Shaw writes T031 (failing idle tests), then:
- Reese: T032 (IdleService), T033 (idle_get_status), T034 (idle_resolve)
- Root: T035 (IdleReturnModal), T036 (wire into Dashboard)
