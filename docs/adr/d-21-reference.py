from fractions import Fraction
from itertools import product

SCALE = 10**100
PI = 31415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679


def multiply(left, right):
    return left * right // SCALE


def divide(left, right):
    return left * SCALE // right


def logarithm(value):
    ratio = divide(value - SCALE, value + SCALE)
    squared = multiply(ratio, ratio)
    power = ratio
    total = 0
    for index in range(3000):
        total += power // (2 * index + 1)
        power = multiply(power, squared)
    return 2 * total


def exponential(value):
    if value < -250 * SCALE:
        return 0
    reduced = value // 1024
    term = total = SCALE
    for index in range(1, 301):
        term = multiply(term, reduced) // index
        total += term
    for index in range(10):
        total = multiply(total, total)
    return total


def sine(value):
    reduced = (value + PI) % (2 * PI) - PI
    term = total = reduced
    for index in range(1, 301):
        term = -multiply(multiply(term, reduced), reduced) // (2 * index * (2 * index + 1))
        total += term
    return total


def number(text):
    return int(Fraction(text) * SCALE)


def probability(target, stop, sigma, days):
    start = -logarithm(SCALE - stop)
    length = logarithm(divide(SCALE + target, SCALE - stop))
    total = divide(stop, target + stop)
    for index in range(1, 4001):
        frequency = divide(index * PI, length)
        square = multiply(frequency, frequency)
        decay = multiply(multiply(sigma, sigma), square) // 2 + multiply(sigma, sigma) // 8
        decay = multiply(decay, days)
        if decay > 250 * SCALE:
            break
        coefficient = divide(
            2 * multiply(frequency, exponential(length // 2) - exponential(-length // 2)) * (-1)**index,
            multiply(multiply(length, exponential(length) - SCALE), SCALE // 4 + square),
        )
        total += multiply(multiply(multiply(exponential(start // 2), coefficient), sine(multiply(frequency, start))), exponential(-decay))
    return total


def decimal_text(value):
    sign = "-" if value < 0 else ""
    absolute = abs(value)
    return sign + str(absolute // SCALE) + "." + str(absolute % SCALE).zfill(100)[:28]


if __name__ == "__main__":
    for target, stop_fraction, sigma, days in product(["0.05", "0.1", "0.2"], ["0.25", "0.5", "0.75"], ["0.01", "0.02", "0.05"], ["5", "20", "120"]):
        stop = multiply(number(target), number(stop_fraction))
        result = probability(number(target), stop, number(sigma), number(days))
        print('("' + '","'.join([target, decimal_text(stop), sigma, days, decimal_text(result)]) + '"),')
