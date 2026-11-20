package survarium.rooms
{
    import flash.display.DisplayObject;
    import flash.display.DisplayObjectContainer;

    public class RoomLayers
    {
        // A room-state reply may open the room after results. The results and
        // their dimmer own the modal layer until closed, regardless of reply order.
        public static function belowResults(room:DisplayObject, results:DisplayObject, dimmer:DisplayObject):void
        {
            var parent:DisplayObjectContainer = room.parent;
            if(parent == null || results == null || !results.visible || results.parent != parent)
                return;
            var ceiling:DisplayObject = dimmer != null && dimmer.parent == parent ? dimmer : results;
            if(parent.getChildIndex(room) > parent.getChildIndex(ceiling))
                parent.setChildIndex(room,parent.getChildIndex(ceiling));
        }
    }
}
