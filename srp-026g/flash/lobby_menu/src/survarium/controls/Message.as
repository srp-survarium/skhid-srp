package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import flash.utils.Dictionary;
   import scaleform.gfx.FocusManager;
   import survarium.messages.MessageBase;
   import survarium.constants.MessageId;
   
   public class Message
   {
      
      protected static var _backSprite:Sprite;
      
      protected static var _stage:*;
      
      protected static var _messages:Object = new Object();
      
      protected static var _instances:Dictionary = new Dictionary();
      
      protected static var _messPosX:int = 0;
      
      protected static var _messPosY:int = 0;
      
      protected static var _messIndex:int = 0;
      
      protected static var _scaleStart:Number = 0.9;
      
      protected static var _currentRenderer:MessageBase = null;
      
      protected static var _hideMessages:Boolean = false;
      
      protected static var _scaleFactorInvalidated:Boolean = false;
      
      protected static var _scaleFactor:Number = 1;
      
      public function Message()
      {
         super();
      }
      
      public static function registerBackground(backClass:Class) : void
      {
         _backSprite = new backClass();
      }
      
      public static function close(messageId:uint, immediately:Boolean = false) : void
      {
         var message:MessageBase = _instances[messageId];
         if(!message)
         {
            return;
         }
         if(!message.closable)
         {
            return;
         }
         stackTrace("cleanUP::messsage " + String(messageId));
         delete _instances[messageId];
         if(message === _currentRenderer)
         {
            _currentRenderer = null;
         }
         var bounds:Rectangle = message.getRect(_stage);
         var offsetX:Number = bounds.width * ((1 - _scaleStart) / 2);
         var offsetY:Number = bounds.height * ((1 - _scaleStart) / 2);
         var targetX:Number = message.x + offsetX;
         var targetY:Number = message.y + offsetY;
         var completeParams:Array;
         trace("close::",message,messageId,"|",completeParams);
         if(immediately)
         {
            removeMessageFromStage(message);
         }
         else
         {
            completeParams = [message];
            TweenLite.to(message,0.1,{
               scale:_scaleFactor * _scaleStart,
               alpha:0,
               x:targetX,
               y:targetY,
               onComplete:removeMessageFromStage,
               onCompleteParams:completeParams
            });
         }
         handleClose();
      }
      
      public static function initStage(stageClip:*) : void
      {
         _stage = stageClip;
         _stage.stage.addEventListener(Event.RESIZE,updateBack,false,-999,true);
      }
      
      protected static function updateBack(event:Event = null) : void
      {
         var needsUpdate:Boolean = _scaleFactorInvalidated || _backSprite && (_backSprite.width != _stage.stage.stageWidth || _backSprite.height != _stage.stage.stageHeight);
         var messageId:*;
         var message:MessageBase;
         trace("Message::handleStageResize::",_scaleFactor,needsUpdate);
         if(!needsUpdate)
         {
            return;
         }
         _scaleFactorInvalidated = false;
         if(_stage && _backSprite && _backSprite.parent)
         {
            _backSprite.width = _stage.stage.stageWidth;
            _backSprite.height = _stage.stage.stageHeight;
         }
         for(messageId in _instances)
         {
            message = _instances[messageId];
            message.scaleX = message.scaleY = _scaleFactor;
            message.y = _stage.stage.stageHeight / 2 - message.getRect(_stage).height / 2;
            message.x = _stage.stage.stageWidth / 2 - message.getRect(_stage).width / 2;
         }
      }
      
      public static function registerMessage(messageType:String, messageClass:Class) : void
      {
         _messages[messageType] = messageClass;
      }
      
      public static function hideMessages() : void
      {
         var messageId:*;
         var message:MessageBase;
         _hideMessages = true;
         for(messageId in _instances)
         {
            message = _instances[messageId];
            message.visible = false;
         }
         if(_backSprite && _backSprite.parent)
         {
            _stage.removeChild(_backSprite);
         }
      }
      
      public static function showMessages() : void
      {
         var messageId:*;
         var message:MessageBase;
         trace("showMessages::",_currentRenderer);
         _hideMessages = false;
         if(!_currentRenderer)
         {
            for(messageId in _instances)
            {
               message = _instances[messageId];
               _currentRenderer = message;
               break;
            }
         }
         if(_currentRenderer)
         {
            _stage.addChild(_backSprite);
            addMessageToStage();
            updateBack();
         }
      }
      
      private static function addMessageToStage() : void
      {
         var targetX:int = 0;
         var targetY:int = 0;
         _currentRenderer.scaleX = _currentRenderer.scaleY = _scaleFactor;
         var bounds:Rectangle = _currentRenderer.getRect(_stage);
         var offsetX:Number;
         var offsetY:Number;
         targetX = _stage.stage.stageWidth / 2 - bounds.width / 2;
         targetY = _stage.stage.stageHeight / 2 - bounds.height / 2;
         if(_currentRenderer.visible == false)
         {
            _currentRenderer.visible = true;
            _currentRenderer.alpha = 0;
            offsetX = bounds.width * ((1 - _scaleStart) / 2);
            offsetY = bounds.height * ((1 - _scaleStart) / 2);
            _currentRenderer.x = targetX + offsetX;
            _currentRenderer.y = targetY + offsetY;
            _currentRenderer.scaleX = _currentRenderer.scaleY = _scaleFactor * _scaleStart;
            trace("MessagePosition:: wh",bounds.width,bounds.height,_currentRenderer.width,_currentRenderer.height,"offset::",offsetX,offsetY);
            _stage.addChild(_currentRenderer);
            TweenLite.to(_currentRenderer,0.1,{
               scale:_scaleFactor,
               alpha:1,
               x:targetX,
               y:targetY
            });
         }
         else
         {
            _currentRenderer.x = targetX;
            _currentRenderer.y = targetY;
            _currentRenderer.scaleX = _currentRenderer.scaleY = _scaleFactor;
            _stage.addChild(_currentRenderer);
         }
      }
      
      public static function showMessage(messageType:String, messageData:* = null, force:Boolean = false, messageId:uint = MessageId.AUTO, keepCurrent:Boolean = false) : *
      {
         trace("MESSAGE::showMessage::",messageId,messageType,force,"cur::",_currentRenderer,"hide::",_hideMessages,_stage.visible);
         if(!_stage)
         {
            return;
         }
         var messageClass:Class = _messages[messageType];
         var id:uint;
         if(!messageClass is Class)
         {
            return;
         }
         if(messageId == MessageId.AUTO)
         {
            id = (_messIndex);
            ++_messIndex;
         }
         else
         {
            id = messageId;
         }
         var message:* = new messageClass();
         trace("messageCreated::",message,messageData,_stage,message as MessageBase,id,"index::",_messIndex,"current::",_currentRenderer,"ff::",FocusManager.getFocus());
         (message as MessageBase).setMessageData(id,messageData);
         message.name = "message_" + String(_messIndex);
         if(messageType == "noclose" || messageType == "debug" || messageType == "ok")
         {
            message.text = messageData;
         }
         _instances[id] = message;
         if((_currentRenderer || _hideMessages) && force == false)
         {
            return message;
         }
         trace("addMessageToStage");
         if(_backSprite)
         {
            _stage.addChild(_backSprite);
         }
         if(_currentRenderer && !keepCurrent)
         {
            _currentRenderer.visible = false;
         }
         _stage.tabEnabled = false;
         _stage.addEventListener(Event.RENDER,onStageRender);
         _currentRenderer = message as MessageBase;
         addMessageToStage();
         updateBack();
         return message;
      }
      
      private static function onStageRender(event:Event) : void
      {
         _stage.removeEventListener(Event.RENDER,onStageRender);
         var renderer:* = _currentRenderer;
         FocusManager.setFocus(renderer);
         FocusManager.setModalClip(renderer);
      }
      
      public static function setScaleFactor(scaleFactor:Number) : void
      {
         trace("Message::setScaleFactor::",scaleFactor);
         if(_scaleFactor == scaleFactor)
         {
            return;
         }
         _scaleFactor = scaleFactor;
         _scaleFactorInvalidated = true;
      }
      
      public static function isAnyMessage() : int
      {
         var messageId:*;
         if(_currentRenderer)
         {
            return _currentRenderer.getMessageId();
         }
         for(messageId in _instances)
         {
            return messageId;
         }
         return -1;
      }
      
      protected static function handleClose() : void
      {
         var nextId:int;
         var back:Sprite;
         var nextMessage:MessageBase;
         if(_stage)
         {
            nextId = isAnyMessage();
            if(nextId == -1 || _hideMessages)
            {
               back = _backSprite;
               _stage.removeChild(_backSprite);
               _currentRenderer = null;
               FocusManager.setFocus(_stage);
               FocusManager.setModalClip(_stage);
            }
            else
            {
               nextMessage = _instances[nextId];
               trace("handleClose::addMessage::",nextId,nextMessage);
               _currentRenderer = nextMessage;
               addMessageToStage();
               FocusManager.setFocus(nextMessage);
               FocusManager.setModalClip(nextMessage);
            }
         }
      }
      
      protected static function removeMessageFromStage(target:* = null) : void
      {
         trace("REMOVE_FROM_STAGE::",target);
         var message:MessageBase = target as MessageBase;
         if(message && _stage && (_stage.contains(message)))
         {
            trace("REMOVE_FROM_STAGE::complete");
            _stage.removeChild(message);
         }
      }
      
      public static function onClick(messageId:int, action:String = null, params:Object = null) : void
      {
         trace("MESSAGE::onClick::",messageId);
         close(messageId);
      }
   }
}

