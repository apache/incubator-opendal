- Proposal Name: `concurrent_stat_in_list`
- Start Date: 2023-11-13
- RFC PR: [apache/opendal#3574](https://github.com/apache/opendal/pull/3574)
- Tracking Issue: [apache/opendal#3575](https://github.com/apache/opendal/issues/3575)

# Summary

Add concurrent stat in list operation.

# Motivation

[RFC-2779](https://github.com/apache/opendal/pull/2779) allows user to list with metakey.
However, the stat inside list could make the list process much slower. We should allow concurrent stat during list so that stat could be sent concurrently.


# Guide-level explanation

For users who want to concurrently run statistics in list operations, they will call the new API `concurrent`. The `concurrent` function will take a number as input, and this number will represent the maximum concurrent stat handlers.

The default behavior remains unchanged, so users using `op.list_with()` are not affected. And this implementation should be zero cost to users who don't want to do concurrent stat.

```rust
op.lister_with(path).metakey(Metakey::ContentLength).concurrent(10).await?
```


# Reference-level explanation

When `concurrent` is set and `list_with` is called, the list operation will be split into two parts:  list and stat.

The list part will iterate through the entries inside the buffer, and if its `metakey` is unknown, it will send a stat request to the storage service.

We will add a new field `concurrent` to `OpList`. The type of `concurrent` is `Option<u32>`. If `concurrent` is `None`, it means the default behavior. If `concurrent` is `Some(n)`, it means the maximum concurrent stat handlers are `n`.

Then we could use a sized `VecDeque` to limit the maximum concurrent stat handlers. Additionally, we could use handlers `JoinHandle<T>` inside `VecDeque` to spawn and queue the stat tasks. 
While iterating through the entries, we should check if the `metakey` is unknown and if the `VecDeque` is full. If the `metakey` is unknown and the `VecDeque` is full, we should wait and join the handle once it’s finished, since we need to keep the entry order.

If the metakey is unknown and the handlers are full, we should break the loop and wait for the spawned tasks inside handlers to finish. After the spawned tasks finish, we should iterate through the handlers and return the result.

If the metakey is known, we should check if the handlers are empty. If true, return the result immediately; otherwise, we should wait for the spawned tasks to finish.

# Drawbacks

1. More memory usage
2. More complex code
3. More complex testing

# Rationale and alternatives

## Why not `VecDeque<BoxFuture<'static, X>>`?

To maintain the order of returned entries, we need to pre-run future entries before returning the current one to address the slowness issue. 
Although we could use `VecDeque<BoxFuture<'static, X>>` to store the spawned tasks, 
using it here would prevent us from executing the async block concurrently when we only have one `cx: &mut Context<'_>`.

## Do we need `Semaphore`?

No, we can control the concurrent number by limiting the length of the `VecDeque`.
Using a `semaphore` will introduce more cost and memory.

## Why not using `JoinSet`?

The main reason is that `JoinSet` can't maintain the order of entries.

The other reason is that `JoinSet` requires mutability to spawn or join the next task, and `tokio::spawn()` requires the async block to be `'static`.
This implies that we need to use `Arc<T>` to wrap our `JoinSet`. However, to change the value inside `Arc`, we need to introduce a `Mutex`. Since it's inside an async block, we need to use Tokio's `Mutex` to satisfy the `Sync` bound. 
Therefore, for every operation on the `JoinSet`, there will be an `.await` on the lock outside the async block, making concurrency impossible inside `poll_next()`.

# Prior art

None

# Unresolved questions

- How to implement a similar logic to `blocking` API? 
   - Quoted from [oowl](https://github.com/oowl): It seems these features can be implemented in blocking mode, but it may require breaking something in OpenDAL, such as using some pthread API in blocking mode.

# Future possibilities

None
