<?php
/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

// describe does not support dataset yet

describe('basic io with fs', function () {
    $op = new \OpenDAL\Operator('fs', ['root' => '/tmp']);

    it('ensure file not exist', function () use ($op) {
        $op->delete('test.txt');
        expect($op->is_exist('test.txt'))->toEqual(0);
    });

    it('write/read file', function () use ($op) {
        $op->write('test.txt', 'hello world');
        expect($op->is_exist('test.txt'))->toEqual(1)
            ->and($op->read('test.txt'))->toEqual('hello world');
    });

    it('write/read file overwrite', function () use ($op) {
        $op->write('test.txt', 'new content');
        expect($op->is_exist('test.txt'))->toEqual(1)
            ->and($op->read('test.txt'))->toEqual('new content');
    });

    it('file metadata', function () use ($op) {
        $meta = $op->stat('test.txt');
        expect($meta)->toBeInstanceOf(\OpenDAL\Metadata::class)
            ->and($meta->content_length)->toEqual(11)
            ->and($meta->mode)->toBeInstanceOf(\OpenDAL\EntryMode::class)
            ->and($meta->mode->is_file)->toEqual(1)
            ->and($meta->mode->is_dir)->toEqual(0);
    });

    it('delete file', function () use ($op) {
        $op->delete('test.txt');
        expect($op->is_exist('test.txt'))->toEqual(0);
    });

    it('create dir', function () use ($op) {
        $op->create_dir('test/');
        expect(is_dir('/tmp/test'))->toBeTrue();
    });
});

describe('basic io with memory', function () {
    $op = new \OpenDAL\Operator('memory', []);

    it('ensure file not exist', function () use ($op) {
        $op->delete('test.txt');
        expect($op->is_exist('test.txt'))->toEqual(0);
    });

    it('write/read file', function () use ($op) {
        $op->write('test.txt', 'hello world');
        expect($op->is_exist('test.txt'))->toEqual(1)
            ->and($op->read('test.txt'))->toEqual('hello world');
    });

    it('write/read file overwrite', function () use ($op) {
        $op->write('test.txt', 'new content');
        expect($op->is_exist('test.txt'))->toEqual(1)
            ->and($op->read('test.txt'))->toEqual('new content');
    });

    it('file metadata', function () use ($op) {
        $meta = $op->stat('test.txt');
        expect($meta)->toBeInstanceOf(\OpenDAL\Metadata::class)
            ->and($meta->content_length)->toEqual(11)
            ->and($meta->mode)->toBeInstanceOf(\OpenDAL\EntryMode::class)
            ->and($meta->mode->is_file)->toEqual(1)
            ->and($meta->mode->is_dir)->toEqual(0);
    });

    it('delete file', function () use ($op) {
        $op->delete('test.txt');
        expect($op->is_exist('test.txt'))->toEqual(0);
    });

    it('create dir', function () use ($op) {
        $op->create_dir('test/');
        expect(is_dir('/tmp/test'))->toBeTrue();
    });
});

describe('binary safe IO with fs', function () {
    $op = new \OpenDAL\Operator('fs', ['root' => '/tmp']);

    it('write & read invalid UTF-8', function () use ($op) {
        $content = "hello 🌰 \x80\x80\x80 🍋";
        $bytesArray = unpack('C*', $content);

        expect($bytesArray)->toBeArray();

        $op->write_binary('test.txt', $bytesArray);
        $content = $op->read('test.txt');

        expect($content)->toBeString()->toEqual($content);
    });
});

describe('binary safe IO with memory', function () {
    $op = new \OpenDAL\Operator('memory', []);

    it('write & read invalid UTF-8', function () use ($op) {
        $content = "hello 🌰 \x80\x80\x80 🍋";
        $bytesArray = unpack('C*', $content);

        expect($bytesArray)->toBeArray();

        $op->write_binary('test.txt', $bytesArray);
        $content = $op->read('test.txt');

        expect($content)->toBeString()->toEqual($content);
    });
});
