package {
    import flash.external.ExternalInterface;
    import flash.utils.getTimer;
    import survarium.controls.SrvContextMenu;
    import survarium.data.LocalizationData;
    import survarium.friends.FriendsMenu;

    // "Join room" in a friend's right-click menu (the owner's of 2026-10-20:
    // "it should also be possible to rmb on friend and join his room, if he
    // is in a room"). The menu opens as before; the lobby is asked which
    // friends are members of a room (`S1;Y`, lobby.md 2.16.3), and if this
    // friend is, the item is added at the menu's end while it is still open
    // ("Queue for room" for a full room). It joins as the Rooms list's Join
    // does (SrpRooms.joinFriend): a PIN asked first, Queue for a full room.
    public class SrpFriendRoom {
        private static const FRIENDS:Array = ["Friends", "Друзі", "Друзья", "Freunde", "Znajomi"];
        private static const JOIN:Array = ["Join room", "Увійти в кімнату", "Войти в комнату",
            "Raum beitreten", "Dołącz do pokoju"];
        private static const QUEUE:Array = ["Queue for room", "Черга в кімнату", "Очередь в комнату",
            "Raum-Warteschlange", "Kolejka do pokoju"];
        // An answer later than this is for a menu long gone.
        private static const WAIT_MS:int = 5000;

        // The menu shown and whose friend it is, waiting for the answer.
        private static var _items:Array = null;
        private static var _friend:Number = 0;
        private static var _askedAt:int = 0;
        // The room its "Join room" joins.
        private static var _room:Object = null;

        // The end of FriendListRender.showContextMenu's menu of a player who
        // is not ignored, just after SrvContextMenu.show(items).
        public static function opened(items:Array, render:Object):void {
            // A hook: nothing of it may throw into the game's code.
            try {
                _items = null;
                var friend:Object = render != null ? render.data : null;
                if (items == null || friend == null || friend.name == null
                        || !FriendsMenu.isFriend(String(friend.name)))
                    return;
                _items = items;
                _friend = Number(friend.id);
                _askedAt = getTimer();
                SrpRooms.friendRooms = answered;
                ExternalInterface.call("request_players_range", SrpRooms.FRIEND_ROOMS, 1);
            } catch (e:Error) {
            }
        }

        // `S1;Y;<count>`, then `<account id>;<key>;<players>;<size>;<pin>` a
        // friend in a room.
        private static function answered(rows:Array):void {
            try {
                var items:Array = _items;
                _items = null;
                if (items == null || getTimer() - _askedAt > WAIT_MS || !showing(items))
                    return;
                for (var i:int = 1; i < rows.length; i++) {
                    var f:Array = String(rows[i].nickname).split(";");
                    if (f.length < 5 || Number(f[0]) != _friend)
                        continue;
                    _room = {"key": int(f[1]), "players": int(f[2]), "size": int(f[3]), "pin": f[4] == "1"};
                    var full:Boolean = _room.players >= _room.size * 2;
                    items.push({"name": say(full ? QUEUE : JOIN), "click_function": join});
                    var menu:SrvContextMenu = SrvContextMenu.getInstance();
                    menu.contextList.data = items;
                    // One row more: still on the screen.
                    if (menu.stage != null && menu.contextList.y + menu.contextList.height > menu.stage.stageHeight)
                        menu.contextList.y = Math.max(4, menu.stage.stageHeight - menu.contextList.height - 4);
                    return;
                }
            } catch (e:Error) {
            }
        }

        // The menu on the screen is still the one of these items.
        private static function showing(items:Array):Boolean {
            var menu:SrvContextMenu = SrvContextMenu.getInstance();
            if (!SrvContextMenu.visisble || menu == null || menu.contextList == null)
                return false;
            var shown:Object = menu.contextList.list.dataProvider;
            if (shown == null || shown.length != items.length)
                return false;
            for (var i:int = 0; i < items.length; i++)
                if (shown[i] !== items[i])
                    return false;
            return true;
        }

        private static function join():void {
            try {
                SrpRooms.joinFriend(_room);
            } catch (e:Error) {
            }
        }

        private static function say(words:Array):String {
            var lang:int = 0;
            var friends:String = LocalizationData.getName("btn_friends");
            for (var i:int = 1; i < FRIENDS.length; i++)
                if (friends == FRIENDS[i])
                    lang = i;
            return String(words[lang]);
        }
    }
}
