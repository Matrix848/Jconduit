import {{ package }}.*;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

public record {{ class_name }}(MemorySegment segment) {
    {% for func in functions %}
    {{ func.signature }} {
        {{ func.body }}
    }
    {% endfor %}
}