package survarium.controls
{
   import com.greensock.TweenLite;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import flash.utils.Dictionary;
   import scaleform.gfx.FocusManager;
   import survarium.messages.MessageBase;
   
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
      
      public static function registerBackground(param1:Class) : void
      {
         _backSprite = new param1();
      }
      
      public static function close(param1:uint, param2:Boolean = false) : void
      {
         var _loc3_:MessageBase = _instances[param1];
         if(!_loc3_)
         {
            return;
         }
         if(!_loc3_.closable)
         {
            return;
         }
         stackTrace("cleanUP::messsage " + String(param1));
         delete _instances[param1];
         if(_loc3_ === _currentRenderer)
         {
            _currentRenderer = null;
         }
         var _loc4_:Rectangle = _loc3_.getRect(_stage);
         var _loc5_:Number = _loc4_.width * ((1 - _scaleStart) / 2);
         var _loc6_:Number = _loc4_.height * ((1 - _scaleStart) / 2);
         var _loc7_:Number = _loc3_.x + _loc5_;
         var _loc8_:Number = _loc3_.y + _loc6_;
         var _loc9_:Array;
         trace("close::",_loc3_,param1,"|",_loc9_);
         if(param2)
         {
            removeMessageFromStage(_loc3_);
         }
         else
         {
            _loc9_ = [_loc3_];
            TweenLite.to(_loc3_,0.1,{
               scale:_scaleFactor * _scaleStart,
               alpha:0,
               x:_loc7_,
               y:_loc8_,
               onComplete:removeMessageFromStage,
               onCompleteParams:_loc9_
            });
         }
         handleClose();
      }
      
      public static function initStage(param1:*) : void
      {
         _stage = param1;
         _stage.stage.addEventListener(Event.RESIZE,updateBack,false,-999,true);
      }
      
      protected static function updateBack(param1:Event = null) : void
      {
         var _loc2_:Boolean = _scaleFactorInvalidated || _backSprite && (_backSprite.width != _stage.stage.stageWidth || _backSprite.height != _stage.stage.stageHeight);
         var _loc3_:*;
         var _loc4_:MessageBase;
         trace("Message::handleStageResize::",_scaleFactor,_loc2_);
         if(!_loc2_)
         {
            return;
         }
         _scaleFactorInvalidated = false;
         if(_stage && _backSprite && _backSprite.parent)
         {
            _backSprite.width = _stage.stage.stageWidth;
            _backSprite.height = _stage.stage.stageHeight;
         }
         for(_loc3_ in _instances)
         {
            _loc4_ = _instances[_loc3_];
            _loc4_.scaleX = _loc4_.scaleY = _scaleFactor;
            _loc4_.y = _stage.stage.stageHeight / 2 - _loc4_.getRect(_stage).height / 2;
            _loc4_.x = _stage.stage.stageWidth / 2 - _loc4_.getRect(_stage).width / 2;
         }
      }
      
      public static function registerMessage(param1:String, param2:Class) : void
      {
         _messages[param1] = param2;
      }
      
      public static function hideMessages() : void
      {
         var _loc1_:*;
         var _loc2_:MessageBase;
         _hideMessages = true;
         for(_loc1_ in _instances)
         {
            _loc2_ = _instances[_loc1_];
            _loc2_.visible = false;
         }
         if(_backSprite && _backSprite.parent)
         {
            _stage.removeChild(_backSprite);
         }
      }
      
      public static function showMessages() : void
      {
         var _loc1_:*;
         var _loc2_:MessageBase;
         trace("showMessages::",_currentRenderer);
         _hideMessages = false;
         if(!_currentRenderer)
         {
            for(_loc1_ in _instances)
            {
               _loc2_ = _instances[_loc1_];
               _currentRenderer = _loc2_;
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
         var _loc1_:int = 0;
         var _loc2_:int = 0;
         _currentRenderer.scaleX = _currentRenderer.scaleY = _scaleFactor;
         var _loc3_:Rectangle = _currentRenderer.getRect(_stage);
         var _loc4_:Number;
         var _loc5_:Number;
         _loc1_ = _stage.stage.stageWidth / 2 - _loc3_.width / 2;
         _loc2_ = _stage.stage.stageHeight / 2 - _loc3_.height / 2;
         if(_currentRenderer.visible == false)
         {
            _currentRenderer.visible = true;
            _currentRenderer.alpha = 0;
            _loc4_ = _loc3_.width * ((1 - _scaleStart) / 2);
            _loc5_ = _loc3_.height * ((1 - _scaleStart) / 2);
            _currentRenderer.x = _loc1_ + _loc4_;
            _currentRenderer.y = _loc2_ + _loc5_;
            _currentRenderer.scaleX = _currentRenderer.scaleY = _scaleFactor * _scaleStart;
            trace("MessagePosition:: wh",_loc3_.width,_loc3_.height,_currentRenderer.width,_currentRenderer.height,"offset::",_loc4_,_loc5_);
            _stage.addChild(_currentRenderer);
            TweenLite.to(_currentRenderer,0.1,{
               scale:_scaleFactor,
               alpha:1,
               x:_loc1_,
               y:_loc2_
            });
         }
         else
         {
            _currentRenderer.x = _loc1_;
            _currentRenderer.y = _loc2_;
            _currentRenderer.scaleX = _currentRenderer.scaleY = _scaleFactor;
            _stage.addChild(_currentRenderer);
         }
      }
      
      public static function showMessage(param1:String, param2:* = null, param3:Boolean = false, param4:uint = 0, param5:Boolean = false) : *
      {
         trace("MESSAGE::showMessage::",param4,param1,param3,"cur::",_currentRenderer,"hide::",_hideMessages,_stage.visible);
         if(!_stage)
         {
            return;
         }
         var _loc6_:Class = _messages[param1];
         var _loc7_:uint;
         if(!_loc6_ is Class)
         {
            return;
         }
         if(param4 == 0)
         {
            _loc7_ = (_messIndex);
            ++_messIndex;
         }
         else
         {
            _loc7_ = param4;
         }
         var _loc8_:* = new _loc6_();
         trace("messageCreated::",_loc8_,param2,_stage,_loc8_ as MessageBase,_loc7_,"index::",_messIndex,"current::",_currentRenderer,"ff::",FocusManager.getFocus());
         (_loc8_ as MessageBase).setMessageData(_loc7_,param2);
         _loc8_.name = "message_" + String(_messIndex);
         if(param1 == "noclose" || param1 == "debug" || param1 == "ok")
         {
            _loc8_.text = param2;
         }
         _instances[_loc7_] = _loc8_;
         if((_currentRenderer || _hideMessages) && param3 == false)
         {
            return _loc8_;
         }
         trace("addMessageToStage");
         if(_backSprite)
         {
            _stage.addChild(_backSprite);
         }
         if(_currentRenderer && !param5)
         {
            _currentRenderer.visible = false;
         }
         _stage.tabEnabled = false;
         _stage.addEventListener(Event.RENDER,onStageRender);
         _currentRenderer = _loc8_ as MessageBase;
         addMessageToStage();
         updateBack();
         return _loc8_;
      }
      
      private static function onStageRender(param1:Event) : void
      {
         _stage.removeEventListener(Event.RENDER,onStageRender);
         var _loc2_:* = _currentRenderer;
         FocusManager.setFocus(_loc2_);
         FocusManager.setModalClip(_loc2_);
      }
      
      public static function setScaleFactor(param1:Number) : void
      {
         trace("Message::setScaleFactor::",param1);
         if(_scaleFactor == param1)
         {
            return;
         }
         _scaleFactor = param1;
         _scaleFactorInvalidated = true;
      }
      
      public static function isAnyMessage() : int
      {
         var _loc1_:*;
         if(_currentRenderer)
         {
            return _currentRenderer.getMessageId();
         }
         for(_loc1_ in _instances)
         {
            return _loc1_;
         }
         return -1;
      }
      
      protected static function handleClose() : void
      {
         var _loc1_:int;
         var _loc2_:Sprite;
         var _loc3_:MessageBase;
         if(_stage)
         {
            _loc1_ = isAnyMessage();
            if(_loc1_ == -1 || _hideMessages)
            {
               _loc2_ = _backSprite;
               _stage.removeChild(_backSprite);
               _currentRenderer = null;
               FocusManager.setFocus(_stage);
               FocusManager.setModalClip(_stage);
            }
            else
            {
               _loc3_ = _instances[_loc1_];
               trace("handleClose::addMessage::",_loc1_,_loc3_);
               _currentRenderer = _loc3_;
               addMessageToStage();
               FocusManager.setFocus(_loc3_);
               FocusManager.setModalClip(_loc3_);
            }
         }
      }
      
      protected static function removeMessageFromStage(param1:* = null) : void
      {
         trace("REMOVE_FROM_STAGE::",param1);
         var _loc2_:MessageBase = param1 as MessageBase;
         if(_loc2_ && _stage && (_stage.contains(_loc2_)))
         {
            trace("REMOVE_FROM_STAGE::complete");
            _stage.removeChild(_loc2_);
         }
      }
      
      public static function onClick(param1:int, param2:String = null, param3:Object = null) : void
      {
         trace("MESSAGE::onClick::",param1);
         close(param1);
      }
   }
}

