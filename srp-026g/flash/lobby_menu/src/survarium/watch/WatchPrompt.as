package survarium.watch
{
   import flash.text.TextFormat;
   import flash.utils.getTimer;
   import survarium.controls.Message;
   import survarium.controls.VictoryScreen;
   import survarium.data.LocalizationData;

   // The prompt over the lobby while a watch is asked for (ReplayAsks): the game's message box with
   // Close alone, told what the ask is doing. Close ends the ask.
   public class WatchPrompt
   {
      private static const ASKING:String = "st_srp_replay_asking";

      // a session's `preparing` (the match server reads the recording)
      private static const OPENING:String = "st_srp_replay_opening";

      private static const CLOSE:String = "st_srp_replay_close";

      // Called when the player closes the prompt.
      public static var onClosed:Function;

      private static var _dialog:Object;

      private static var _data:Object;

      private static var _shownKey:String;

      private static var _shownText:String = "";

      // the ask's start, for the seconds `preparing` shows
      private static var _preparingSince:int = 0;

      public function WatchPrompt()
      {
         super();
      }

      public static function asking() : void
      {
         show(ASKING);
      }

      // The prompt with `key`'s text and Close alone.
      public static function show(key:String) : void
      {
         var text:String = key == OPENING ? preparingText() : LocalizationData.getName(key);
         drop();
         _shownText = text;
         _data = {
            "text":text,
            "label_ok":null,
            "label_cancel":LocalizationData.getName(CLOSE),
            "onYes":closedByPlayer,
            "onNo":closedByPlayer
         };
         _shownKey = key;
         try
         {
            // Over the results (their Watch replay), the message layer they hide.
            var results:VictoryScreen = LobbyMenu.id != null ? LobbyMenu.id.victory_screen : null;
            if(results != null)
            {
               results.showOver();
            }
            var message:Object = Message.showMessage("apply",_data,true);
            _dialog = message;
            if(message)
            {
               if(text.length > 120 && message.textField)
               {
                  var format:TextFormat = message.textField.defaultTextFormat;
                  format.size = 16;
                  message.textField.defaultTextFormat = format;
               }
               message.validateNow();
               if(message.btnAccept && message.btnCancel && message.background)
               {
                  message.btnAccept.visible = false;
                  message.btnAccept.enabled = false;
                  message.btnCancel.x = int(message.background.x + (message.background.width - message.btnCancel.width) / 2);
               }
            }
         }
         catch(e:Error)
         {
         }
      }

      // `preparing`: the seconds since the ask, in place when the prompt shows them already.
      public static function preparing(since:int) : void
      {
         _preparingSince = since;
         var text:String = preparingText();
         var message:Object = _dialog;
         if(_shownKey != OPENING || !message || !message.textField)
         {
            show(OPENING);
            return;
         }
         if(text == _shownText)
         {
            return;
         }
         try
         {
            var high:Number = message.textField.textHeight;
            message.textField.text = text;
            _shownText = text;
            if(message.textField.textHeight != high)
            {
               show(OPENING);
            }
         }
         catch(e:Error)
         {
            show(OPENING);
         }
      }

      private static function preparingText() : String
      {
         var seconds:int = Math.max(0,int((getTimer() - _preparingSince) / 1000));
         return LocalizationData.format(OPENING,LocalizationData.format("st_srp_replay_waiting",seconds));
      }

      // The prompt gone, without a word (its ask ended with nothing to say).
      public static function close() : void
      {
         drop();
      }

      private static function drop() : void
      {
         var message:Object = _dialog;
         if(_data)
         {
            _data.onYes = null;
            _data.onNo = null;
         }
         _dialog = null;
         _data = null;
         _shownKey = null;
         if(message)
         {
            try
            {
               Message.close(uint(message.getMessageId()),true);
            }
            catch(e:Error)
            {
            }
         }
      }

      private static function closedByPlayer(data:Object = null) : void
      {
         _dialog = null;
         _data = null;
         _shownKey = null;
         if(onClosed != null)
         {
            onClosed();
         }
      }
   }
}
