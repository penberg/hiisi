use criterion::async_executor::FuturesExecutor;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use inferno::manager;
use pprof::criterion::{Output, PProfProfiler};
use std::rc::Rc;

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("inferno");
    group.throughput(Throughput::Elements(1));

    let manager = Rc::new(manager::ResourceManager::new());
    group.bench_function("execute", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            let exec_req =
                inferno::proto::StreamRequest::Execute(inferno::proto::ExecuteStreamReq {
                    stmt: inferno::proto::Stmt {
                        sql: Some("SELECT 1".to_string()),
                        sql_id: None,
                        args: vec![],
                        named_args: vec![],
                        want_rows: None,
                        replication_index: None,
                    },
                });
            let req = inferno::proto::PipelineReqBody {
                baton: None,
                requests: vec![exec_req],
            };
            inferno::executor::execute_client_req(manager.clone(), req, "test").await;
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench
}
criterion_main!(benches);
