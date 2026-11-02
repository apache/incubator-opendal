import 'package:test/test.dart';
import 'dart:typed_data';
import '../lib/opendal.dart';

void main() {
  group('opendal unit test', () {
    group('opendal fs schema', () {
      test('File and Directory functions in fs schema', () async {
        final storage = await Storage.init(schemeStr: "fs", map: {"root": "/tmp"});
        final File = storage.initFile();
        var testFile = File("test_1.txt");
        expect(await testFile.exists(), false);

        var anotherFile = File("test.txt");
        expect(await anotherFile.exists(), false);

        final Directory = storage.initDir();

        var testDir = Directory("test_dir/");
        await testDir.create();
        expect(await testDir.exists(), true);
      });
    });

    group('opendal memory schema', () {
      test('File and Directory functions in memory schema', () async {
        final storage = await Storage.init(schemeStr: "memory", map: {"root": "/tmp"});
        final Directory = storage.initDir();

        var testDir = Directory("test/");
        await testDir.create();
        expect(
            await testDir.exists(), isTrue); // Directory exists after creation

        final meta = await testDir.stat();
        expect(meta, isNotNull);
        expect(meta.isFile, false);
        expect(meta.isDirectory, true);
      });

      test('File read/write round-trip', () async {
        final storage = await Storage.init(schemeStr: "memory", map: {"root": "/tmp"});
        final File = storage.initFile();
        var testFile = File("roundtrip.txt");
        final data = Uint8List.fromList([1, 2, 3, 4, 5]);

        await testFile.write(data);
        expect(await testFile.exists(), true);

        final readData = await testFile.read();
        expect(readData, equals(data));
      });

      test('File read from missing path throws', () async {
        final storage = await Storage.init(schemeStr: "memory", map: {"root": "/tmp"});
        final File = storage.initFile();
        var missingFile = File("nonexistent.txt");

        expect(() async => await missingFile.read(), throwsA(anything));
      });

      test('File overwrite semantics', () async {
        final storage = await Storage.init(schemeStr: "memory", map: {"root": "/tmp"});
        final File = storage.initFile();
        var testFile = File("overwrite.txt");

        await testFile.write(Uint8List.fromList([1, 2, 3]));
        await testFile.write(Uint8List.fromList([4, 5]));

        final readData = await testFile.read();
        expect(readData, equals(Uint8List.fromList([4, 5])));
      });
    });
  });
}
