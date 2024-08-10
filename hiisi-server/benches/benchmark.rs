use criterion::async_executor::FuturesExecutor;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use hiisi::manager;
use pprof::criterion::{Output, PProfProfiler};
use std::rc::Rc;

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("hiisi");
    group.throughput(Throughput::Elements(1));

    let manager = Rc::new(manager::ResourceManager::new());
    group.bench_function("execute", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            let exec_req = hiisi::proto::StreamRequest::Execute(hiisi::proto::ExecuteStreamReq {
                stmt: hiisi::proto::Stmt {
                    sql: Some("SELECT 1".to_string()),
                    sql_id: None,
                    args: vec![],
                    named_args: vec![],
                    want_rows: None,
                    replication_index: None,
                },
            });
            let req = hiisi::proto::PipelineReqBody {
                baton: None,
                requests: vec![exec_req],
            };
            let req = hiisi::executor::Request {
                database: "test".to_string(),
                req,
            };
            hiisi::executor::execute_client_req(manager.clone(), req)
                .await
                .unwrap();
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench
}
criterion_main!(benches);
