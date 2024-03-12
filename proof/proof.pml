// Parent ActorCell
bool stopped_p;
chan drop_p = [1] of { byte };
chan result_p = [1] of { byte };
mtype recv_p;

bool has_child_p;

// Child ActorCell
bool stopped_c;
chan drop_c = [1] of { byte };
chan result_c = [1] of { byte };
mtype recv_c;

mtype = {None, Supervision};

#define MAX_COUNT 5

// Parent

proctype actor_cell_p(chan resolve) {
    do
    ::
        if
        :: stopped_p ->
            if
            :: has_child_p ->
                stopped_c = true;

                result_c ? _;

                has_child_p = false
            :: else
            fi

            resolve ! None;

            break
        :: else ->
            if
            :: result_c ? [_] ->
                result_c ? _;

                has_child_p = false;

                resolve ! Supervision;

                break
            :: else
            fi
        fi
    od
}

proctype actor_future_p(chan fut_done_p) {
    chan resolve = [0] of { mtype };

    int i;

    for (i : 1 .. MAX_COUNT) {
        if
        :: true -> skip
        :: true ->
            run actor_cell_p(resolve);
            resolve ? recv_p
        :: !stopped_p && !has_child_p ->
            run spawn_actor_c()
            has_child_p = true;
        ::  stopped_p &&  has_child_p ->
            stopped_c = true
        fi
    }

    bool dropped;

    if
    :: true ->
        run drop_cell_p();
        dropped = true
    :: true
    fi

    for (i : 1 .. MAX_COUNT) {
        if
        :: true -> skip
        :: !dropped ->
            run actor_cell_p(resolve);
            resolve ? recv_p
        fi
    }

    if
    :: !dropped ->
        run drop_cell_p();
        dropped = true
    :: else
    fi

    fut_done_p ! 1
}

proctype drop_cell_p() {
    if
    :: has_child_p ->
        stopped_c = true;

        result_c ? _;

        has_child_p = false
    :: else
    fi

    drop_p ! 1
}

proctype spawn_actor_p() {
    chan fut_done_p = [1] of { byte };

    run actor_future_p(fut_done_p);

    fut_done_p ? _;
    drop_p ? _;

    result_p ! 1
}

// Child

proctype actor_cell_c(chan resolve) {
    do
    :: if
        :: stopped_c ->
            resolve ! None;

            break
        :: else
        fi
    od
}

proctype actor_future_c(chan fut_done_c) {
    chan resolve = [0] of { mtype };

    if
    :: true ->
        run actor_cell_c(resolve);
        resolve ? recv_c;
    :: true
    fi

    skip;

    run drop_cell_c();

    skip;

    fut_done_c ! 1
}

proctype drop_cell_c() {
    drop_c ! 1
}

proctype spawn_actor_c() {
    chan fut_done_c = [1] of { byte };

    run actor_future_c(fut_done_c);
    fut_done_c ? _;

    drop_c ? _;

    result_c ! 1
}

init {
    run spawn_actor_p();

    stopped_p = true;

    result_p ? _
}

ltl p1 {
    [] (
        (recv_p == None) -> X(recv_p == None)
    )
}

ltl p2 {
    [] (
        (recv_c == None) -> X(recv_c == None)
    )
}
