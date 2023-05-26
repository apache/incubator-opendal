#!/usr/bin/env python3

from argparse import ArgumentDefaultsHelpFormatter, ArgumentParser
from pathlib import Path
import shutil
import subprocess


def classifier_to_target(classifier: str) -> str:
    if classifier == 'osx-aarch_64':
        return 'aarch64-apple-darwin'
    if classifier == 'osx-x86_64':
        return 'x86_64-apple-darwin'
    raise Exception(f'unsupproted classifier {classifier}')


def get_cargo_artifact_name(classifier: str) -> str:
    if classifier == 'osx-aarch_64':
        return 'libopendal_java.dylib'
    if classifier == 'osx-x86_64':
        return 'libopendal_java.dylib'
    raise Exception(f'unsupproted classifier {classifier}')


if __name__ == '__main__':
    basedir = Path(__file__).parent.parent

    parser = ArgumentParser(formatter_class=ArgumentDefaultsHelpFormatter)
    parser.add_argument('--classifier', type=str, required=True)
    args = parser.parse_args()

    cmd = ['cargo', 'build', '--color=always', '--release']

    target = classifier_to_target(args.classifier)
    if target:
        command = ['rustup', 'target', 'add', target]
        print(subprocess.list2cmdline(command))
        subprocess.run(command, cwd=basedir, check=True)
        cmd += ['--target', target]

    output = basedir / 'target' / 'bindings'
    Path(output).mkdir(exist_ok=True, parents=True)
    cmd += ['--target-dir', output]

    print(subprocess.list2cmdline(cmd))
    subprocess.run(cmd, cwd=basedir, check=True)

    artifact = get_cargo_artifact_name(args.classifier)
    src = output / target / 'release' / artifact
    dst = basedir / 'target' / 'classes' / 'native' / args.classifier / artifact
    dst.parent.mkdir(exist_ok=True, parents=True)
    shutil.copy2(src, dst)
