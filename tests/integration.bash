#!/bin/bash

BASEDIR=$(mktemp -d)
FAIL_COUNT=0
FAILED_TESTS=""


echo "Compiling..."
cargo build
if [ $? -ne 0 ]; then
  exit 1;
fi
cp target/debug/hashfilter $BASEDIR


echo "Preparing..."
echo "cd $BASEDIR"
cd $BASEDIR

mkdir environment
cd environment

mkdir test
mkdir secondsecond

for i in $(seq 1 10); do
  dd bs=1M count=50 if=/dev/zero of=test/little_$i
  dd bs=1M count=50 if=/dev/zero of=secondsecond/little_$i
done

for i in $(seq 1 5); do
  dd bs=1M count=200 if=/dev/zero of=test/middle_$i
  dd bs=1M count=200 if=/dev/zero of=secondsecond/middle_$i
done

dd bs=1M count=500 if=/dev/zero of=secondsecond/big_1


echo "Testing Update..."

echo "hashfilter -u"
../hashfilter -u
LINES=$(wc -l sha1sum.txt | cut -d' ' -f1)
if [[ ! -e sha1sum.txt ]] || [[ ! $LINES -eq 31 ]]; then
  echo "Update FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Update\n"
fi

echo "sed -i '$ d' sha1sum.txt"
sed -i '$ d' sha1sum.txt
LINES=$(wc -l sha1sum.txt | cut -d' ' -f1)
if [[ ! $LINES -eq 30 ]]; then
  echo "sed FAILED"
  exit 1
fi

echo "hashfilter -u"
../hashfilter -u
LINES=$(wc -l sha1sum.txt | cut -d' ' -f1)
if [[ ! $LINES -eq 31 ]]; then
  echo "Update-2 FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Update-2\n"
fi


echo "Testing Verify..."

echo "hashfilter -v"
OUTPUT=$(../hashfilter -v)
echo -e "$OUTPUT"
if [[ $OUTPUT =~ .*FAILED.* ]]; then
  echo "Verify FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Verify\n"
fi

rm -f known_good*
rm -f to_check*

echo "sed -i 's/./0/' sha1sum.txt"
sed -i 's/./0/' sha1sum.txt

echo "hashfilter -v"
OUTPUT=$(../hashfilter -v)
echo -e "$OUTPUT"
if [[ ! $OUTPUT =~ .*FAILED.* ]]; then
  echo "Verify-2 FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Verify-2\n"
fi

rm -f sha1sum.txt
rm -f known_good*
rm -f to_check*


echo "Testing Update-Subdir..."

echo "hashfilter -us"
../hashfilter -us
LINES_TEST=$(wc -l test/sha1sum.txt | cut -d' ' -f1)
LINES_SECOND=$(wc -l secondsecond/sha1sum.txt | cut -d' ' -f1)
if [[ ! -e test/sha1sum.txt ]] || [[ ! -e secondsecond/sha1sum.txt ]] || [[ ! $LINES_TEST -eq 15 ]] || [[ ! $LINES_SECOND -eq 16 ]]; then
  echo "Update-Subdir FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Update-Subdir\n"
fi

echo "sed -i '$ d' test/sha1sum.txt"
sed -i '$ d' test/sha1sum.txt
LINES=$(wc -l test/sha1sum.txt | cut -d' ' -f1)
if [[ ! $LINES -eq 14 ]]; then
  echo "sed FAILED"
  exit 1
fi

echo "hashfilter -us"
../hashfilter -us
LINES_TEST=$(wc -l test/sha1sum.txt | cut -d' ' -f1)
LINES_SECOND=$(wc -l secondsecond/sha1sum.txt | cut -d' ' -f1)
if [[ ! -e test/sha1sum.txt ]] || [[ ! -e secondsecond/sha1sum.txt ]] || [[ ! $LINES_TEST -eq 15 ]] || [[ ! $LINES_SECOND -eq 16 ]]; then
  echo "Update-Subdir-2 FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Update-Subdir-2\n"
fi


echo "Testing Verify-Subdir..."

echo "hashfilter -vs"
OUTPUT=$(../hashfilter -vs)
echo -e "$OUTPUT"
FIND=$(find . -iname 'known_good*')
if [[ $OUTPUT =~ .*FAILED.* ]] || [[  -z $FIND ]]; then
  echo "Verify-Subdir FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Verify-Subdir\n"
fi

rm -f known_good*
rm -f to_check*

echo "sed -i 's/./0/' test/sha1sum.txt"
sed -i 's/./0/' test/sha1sum.txt

echo "hashfilter -vs"
OUTPUT=$(../hashfilter -vs)
echo -e "$OUTPUT"
FIND=$(find . -iname 'to_check*')
FIND_LINES=$(echo "$FIND" | wc -l | cut -d' ' -f1)
if [[ ! $OUTPUT =~ .*FAILED.* ]] || [[ $FIND_LINES -ne 2 ]]; then
  echo "Verify-Subdir-2 FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Verify-Subdir-2\n"
fi

rm -f known_good*
rm -f to_check*


echo "Preparing Update-Subdir-Ignore..."
echo "Creating 'ignore' directory and appending it to .hfignore"
mkdir ignore
touch ignore/test1
touch ignore/test2
echo 'ignore' >> .hfignore

echo "Testing Update-Subdir-Ignore..."
../hashfilter -us
if [[ -e ignore/sha1sum.txt ]]; then
  echo "Update-Subdir-Ignore FAILED"
  ((FAIL_COUNT++))
  FAILED_TESTS+="Update-Subdir-Ignore\n"
fi


echo "Testing Verify-Subdir-Progress..."
echo "You now need to visually check for uglyness. Feel free to press some Keys while the program is running, as they should not be echoed. Press Enter when you are ready."
read

echo "hashfilter -vs --loglevel=progress"
../hashfilter -vs --loglevel=progress

echo -e "\n\nNumber of failed tests: $FAIL_COUNT"
if [[ ! -z $FAILED_TESTS ]]; then
  echo "Tests that failed:"
  echo -e "$FAILED_TESTS"
fi

echo -e "\n\nNOTE: The progress test should FAIL on test and OK on secondsecond, as the integration test altered test/sha1sum.txt"

echo "Tests completed. Press Enter to clean up environment."
read

cd
rm -rf $BASEDIR
