import * as Comlink from 'comlink';

export class WorkerPoolManager<T> {
  private idleWorkers: Comlink.Remote<T>[];

  /**
   * Populated when there are no idle workers.
   */
  private workQueue: [
    work: (worker: Comlink.Remote<T>) => any | Promise<any>,
    cb: (ret: any) => void
  ][] = [];

  constructor(workers: Comlink.Remote<T>[]) {
    this.idleWorkers = workers;
  }

  public submitWork = async <R>(
    work: (worker: Comlink.Remote<T>) => R | Promise<R>
  ): Promise<R> => {
    if (this.idleWorkers.length === 0) {
      return new Promise<R>(resolve => {
        this.workQueue.push([work, resolve]);
      });
    }

    const worker = this.idleWorkers.pop()!;

    let ret: { type: 'ok'; val: R } | { type: 'err'; err: any };

    try {
      const out = await work(worker);
      ret = { type: 'ok', val: out };
    } catch (err) {
      console.error('Error in worker', err);
      ret = { type: 'err', err };
    } finally {
      this.idleWorkers.push(worker);
    }

    if (this.workQueue.length > 0) {
      const [nextWork, cb] = this.workQueue.shift()!;
      this.submitWork(nextWork).then(cb);
    }

    if (ret.type === 'err') {
      throw ret.err;
    }
    return ret.val;
  };
}

let workers: Promise<WorkerPoolManager<any>> | null = null;

const clamp = (x: number, min: number, max: number) => Math.min(Math.max(x, min), max);

export const getWorkers = async () => {
  if (workers) {
    return workers;
  }

  // const wasmBytesAB = await fetch('/engine_bg.wasm').then(r => r.arrayBuffer());
  // const wasmBytes = new Uint8Array(wasmBytesAB);

  const workerMod = await import('./wasmWorker.worker?worker');

  workers = new Promise(resolve => {
    const numWorkers = clamp((navigator.hardwareConcurrency || 4) - 2, 1, 512);
    const workers = Array.from({ length: numWorkers }, () => {
      const worker = new workerMod.default();
      const wrapped = Comlink.wrap<any>(worker);
      // wrapped.setWasmBytes(wasmBytes);
      return wrapped;
    });
    resolve(new WorkerPoolManager(workers));
  });
  return workers;
};
